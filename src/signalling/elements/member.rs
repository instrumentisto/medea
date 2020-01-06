//! [`Member`] is member of [`Room`].
//!
//! [`Room`]: crate::signalling::room::Room

use std::{
    cell::RefCell,
    collections::HashMap,
    convert::TryFrom as _,
    rc::{Rc, Weak},
};

use derive_more::Display;
use failure::Fail;
use medea_client_api_proto::{IceServer, PeerId};
use medea_control_api_proto::grpc::api::{
    Element as RootElementProto, Member as MemberProto,
    Room_Element as ElementProto,
};

use crate::{
    api::control::{
        callback::url::CallbackUrl,
        endpoints::WebRtcPlayEndpoint as WebRtcPlayEndpointSpec,
        refs::{Fid, StatefulFid, ToEndpoint, ToMember, ToRoom},
        EndpointId, MemberId, MemberSpec, RoomId, RoomSpec,
        TryFromElementError, WebRtcPlayId, WebRtcPublishId,
    },
    log::prelude::*,
    media::IceUser,
};

use super::endpoints::{
    webrtc::{WebRtcPlayEndpoint, WebRtcPublishEndpoint},
    Endpoint,
};

/// Errors which may occur while loading [`Member`]s from [`RoomSpec`].
#[derive(Debug, Display, Fail)]
pub enum MembersLoadError {
    /// Errors that can occur when we try transform some spec from `Element`.
    #[display(fmt = "TryFromElementError: {}", _0)]
    TryFromError(TryFromElementError, StatefulFid),

    /// [`Member`] not found.
    #[display(fmt = "Member [id = {}] not found", _0)]
    MemberNotFound(Fid<ToMember>),

    /// [`EndpointSpec`] not found.
    ///
    /// [`EndpointSpec`]: crate::api::control::endpoints::EndpointSpec
    #[display(
        fmt = "Endpoint [id = {}] was referenced but not found in spec",
        _0
    )]
    EndpointNotFound(String),
}

#[allow(clippy::pub_enum_variant_names)]
#[derive(Debug, Fail, Display)]
pub enum MemberError {
    #[display(fmt = "Endpoint [id = {}] not found.", _0)]
    EndpointNotFound(Fid<ToEndpoint>),
}

/// [`Member`] is member of [`Room`].
///
/// [`Room`]: crate::signalling::room::Room
#[derive(Clone, Debug)]
pub struct Member(Rc<RefCell<MemberInner>>);

#[derive(Debug)]
struct MemberInner {
    /// [`RoomId`] of [`Room`] to which this [`Member`] relates.
    room_id: RoomId,

    /// ID of this [`Member`].
    id: MemberId,

    /// All [`WebRtcPublishEndpoint`]s of this [`Member`].
    srcs: HashMap<WebRtcPublishId, WebRtcPublishEndpoint>,

    /// All [`WebRtcPlayEndpoint`]s of this [`Member`].
    sinks: HashMap<WebRtcPlayId, WebRtcPlayEndpoint>,

    /// Credentials for this [`Member`].
    credentials: String,

    /// [`IceUser`] of this [`Member`].
    ice_user: Option<IceUser>,

    /// URL to which `on_join` Control API callback will be sent.
    on_join: Option<CallbackUrl>,

    /// URL to which `on_leave` Control API callback will be sent.
    on_leave: Option<CallbackUrl>,
}

impl Member {
    /// Creates new empty [`Member`].
    ///
    /// To fill this [`Member`], you need to call [`Member::load`]
    /// function.
    pub fn new(id: MemberId, credentials: String, room_id: RoomId) -> Self {
        Self(Rc::new(RefCell::new(MemberInner {
            id,
            srcs: HashMap::new(),
            sinks: HashMap::new(),
            credentials,
            ice_user: None,
            room_id,
            on_leave: None,
            on_join: None,
        })))
    }

    /// Lookups [`MemberSpec`] by [`MemberId`] from [`MemberSpec`].
    ///
    /// Returns [`MembersLoadError::MemberNotFound`] when member not found.
    ///
    /// Returns [`MembersLoadError::TryFromError`] when found element which is
    /// not [`MemberSpec`].
    fn get_member_from_room_spec(
        &self,
        room_spec: &RoomSpec,
        member_id: &MemberId,
    ) -> Result<MemberSpec, MembersLoadError> {
        let element = room_spec.pipeline.get(member_id).map_or(
            Err(MembersLoadError::MemberNotFound(Fid::<ToMember>::new(
                self.room_id(),
                member_id.clone(),
            ))),
            Ok,
        )?;

        MemberSpec::try_from(element).map_err(|e| {
            MembersLoadError::TryFromError(
                e,
                Fid::<ToMember>::new(self.room_id(), member_id.clone()).into(),
            )
        })
    }

    /// Loads all sources and sinks of this [`Member`].
    fn load(
        &self,
        room_spec: &RoomSpec,
        store: &HashMap<MemberId, Self>,
    ) -> Result<(), MembersLoadError> {
        let self_id = self.id();

        let this_member_spec =
            self.get_member_from_room_spec(room_spec, &self_id)?;

        let this_member = store
            .get(&self.id())
            .ok_or_else(|| MembersLoadError::MemberNotFound(self.get_fid()))?;

        this_member.set_callback_urls(&this_member_spec);

        for (spec_play_name, spec_play_endpoint) in
            this_member_spec.play_endpoints()
        {
            let publisher_id =
                MemberId(spec_play_endpoint.src.member_id.to_string());
            let publisher_member =
                store.get(&publisher_id).ok_or_else(|| {
                    MembersLoadError::MemberNotFound(Fid::<ToMember>::new(
                        self.room_id(),
                        publisher_id,
                    ))
                })?;
            let publisher_spec = self.get_member_from_room_spec(
                room_spec,
                &spec_play_endpoint.src.member_id,
            )?;

            let publisher_endpoint = publisher_spec
                .get_publish_endpoint_by_id(
                    spec_play_endpoint.src.endpoint_id.clone(),
                )
                .ok_or_else(|| {
                    MembersLoadError::EndpointNotFound(
                        spec_play_endpoint.src.endpoint_id.to_string(),
                    )
                })?;

            if let Some(publisher) = publisher_member.get_src_by_id(
                &spec_play_endpoint.src.endpoint_id.to_string().into(),
            ) {
                let new_play_endpoint = WebRtcPlayEndpoint::new(
                    spec_play_name,
                    spec_play_endpoint.src.clone(),
                    publisher.downgrade(),
                    this_member.downgrade(),
                );

                self.insert_sink(new_play_endpoint.clone());

                publisher.add_sink(new_play_endpoint.downgrade());
            } else {
                let new_publish_id = spec_play_endpoint.src.endpoint_id.clone();
                let new_publish = WebRtcPublishEndpoint::new(
                    new_publish_id,
                    publisher_endpoint.p2p,
                    publisher_member.downgrade(),
                    publisher_endpoint.is_relay,
                );

                let new_self_play = WebRtcPlayEndpoint::new(
                    spec_play_name,
                    spec_play_endpoint.src.clone(),
                    new_publish.downgrade(),
                    this_member.downgrade(),
                );

                new_publish.add_sink(new_self_play.downgrade());

                publisher_member.insert_src(new_publish);

                self.insert_sink(new_self_play);
            }
        }

        // This is necessary to create [`WebRtcPublishEndpoint`],
        // to which none [`WebRtcPlayEndpoint`] refers.
        this_member_spec
            .publish_endpoints()
            .filter(|(endpoint_id, _)| self.srcs().get(endpoint_id).is_none())
            .for_each(|(endpoint_id, e)| {
                self.insert_src(WebRtcPublishEndpoint::new(
                    endpoint_id,
                    e.p2p,
                    this_member.downgrade(),
                    e.is_relay,
                ));
            });

        Ok(())
    }

    /// Returns [`Fid`] to this [`Member`].
    pub fn get_fid(&self) -> Fid<ToMember> {
        Fid::<ToMember>::new(self.room_id(), self.id())
    }

    /// Returns [`Fid`] to some endpoint from this [`Member`].
    ///
    /// __Note__ this function don't check presence of `Endpoint` in this
    /// [`Member`].
    pub fn get_fid_to_endpoint(
        &self,
        endpoint_id: EndpointId,
    ) -> Fid<ToEndpoint> {
        Fid::<ToEndpoint>::new(self.room_id(), self.id(), endpoint_id)
    }

    /// Notifies [`Member`] that some [`Peer`]s removed.
    ///
    /// All [`PeerId`]s related to this [`Member`] will be removed.
    ///
    /// [`Peer`]: crate::media::peer::Peer
    pub fn peers_removed(&self, peer_ids: &[PeerId]) {
        self.srcs()
            .values()
            .for_each(|p| p.remove_peer_ids(peer_ids));

        self.sinks()
            .values()
            .filter_map(|p| p.peer_id().map(|id| (id, p)))
            .filter(|(id, _)| peer_ids.contains(&id))
            .for_each(|(_, p)| p.reset());
    }

    /// Returns list of [`IceServer`] for this [`Member`].
    pub fn servers_list(&self) -> Option<Vec<IceServer>> {
        self.0.borrow().ice_user.as_ref().map(IceUser::servers_list)
    }

    /// Returns and sets to `None` [`IceUser`] of this [`Member`].
    pub fn take_ice_user(&self) -> Option<IceUser> {
        self.0.borrow_mut().ice_user.take()
    }

    /// Replaces and returns [`IceUser`] of this [`Member`].
    pub fn replace_ice_user(&self, new_ice_user: IceUser) -> Option<IceUser> {
        self.0.borrow_mut().ice_user.replace(new_ice_user)
    }

    /// Returns [`MemberId`] of this [`Member`].
    pub fn id(&self) -> MemberId {
        self.0.borrow().id.clone()
    }

    /// Returns credentials of this [`Member`].
    pub fn credentials(&self) -> String {
        self.0.borrow().credentials.clone()
    }

    /// Returns all srcs of this [`Member`].
    pub fn srcs(&self) -> HashMap<WebRtcPublishId, WebRtcPublishEndpoint> {
        self.0.borrow().srcs.clone()
    }

    /// Returns all sinks endpoints of this [`Member`].
    pub fn sinks(&self) -> HashMap<WebRtcPlayId, WebRtcPlayEndpoint> {
        self.0.borrow().sinks.clone()
    }

    /// Inserts sink endpoint into this [`Member`].
    pub fn insert_sink(&self, endpoint: WebRtcPlayEndpoint) {
        self.0.borrow_mut().sinks.insert(endpoint.id(), endpoint);
    }

    /// Inserts source endpoint into this [`Member`].
    pub fn insert_src(&self, endpoint: WebRtcPublishEndpoint) {
        self.0.borrow_mut().srcs.insert(endpoint.id(), endpoint);
    }

    /// Lookups [`WebRtcPublishEndpoint`] source endpoint by
    /// [`WebRtcPublishId`].
    pub fn get_src_by_id(
        &self,
        id: &WebRtcPublishId,
    ) -> Option<WebRtcPublishEndpoint> {
        self.0.borrow().srcs.get(id).cloned()
    }

    /// Lookups [`WebRtcPlayEndpoint`] sink endpoint by [`WebRtcPlayId`].
    pub fn get_sink_by_id(
        &self,
        id: &WebRtcPlayId,
    ) -> Option<WebRtcPlayEndpoint> {
        self.0.borrow().sinks.get(id).cloned()
    }

    /// Removes sink [`WebRtcPlayEndpoint`] from this [`Member`].
    pub fn remove_sink(&self, id: &WebRtcPlayId) {
        self.0.borrow_mut().sinks.remove(id);
    }

    /// Removes source [`WebRtcPublishEndpoint`] from this [`Member`].
    pub fn remove_src(&self, id: &WebRtcPublishId) {
        self.0.borrow_mut().srcs.remove(id);
    }

    /// Takes sink from [`Member`]'s `sinks`.
    pub fn take_sink(&self, id: &WebRtcPlayId) -> Option<WebRtcPlayEndpoint> {
        self.0.borrow_mut().sinks.remove(id)
    }

    /// Takes src from [`Member`]'s `srsc`.
    pub fn take_src(
        &self,
        id: &WebRtcPublishId,
    ) -> Option<WebRtcPublishEndpoint> {
        self.0.borrow_mut().srcs.remove(id)
    }

    /// Returns [`RoomId`] of this [`Member`].
    pub fn room_id(&self) -> RoomId {
        self.0.borrow().room_id.clone()
    }

    /// Creates new [`WebRtcPlayEndpoint`] based on provided
    /// [`WebRtcPlayEndpointSpec`].
    ///
    /// This function will add created [`WebRtcPlayEndpoint`] to `src`s of
    /// [`WebRtcPublishEndpoint`] and to provided [`Member`].
    pub fn create_sink(
        member: &Rc<Self>,
        id: WebRtcPlayId,
        spec: WebRtcPlayEndpointSpec,
    ) {
        let src = member.get_src_by_id(&spec.src.endpoint_id).unwrap();

        let sink = WebRtcPlayEndpoint::new(
            id,
            spec.src,
            src.downgrade(),
            member.downgrade(),
        );

        src.add_sink(sink.downgrade());
        member.insert_sink(sink);
    }

    /// Lookups [`WebRtcPublishEndpoint`] and [`WebRtcPlayEndpoint`] at one
    /// moment by ID.
    pub fn get_endpoint_by_id(
        &self,
        id: String,
    ) -> Result<Endpoint, MemberError> {
        let webrtc_publish_id = id.into();
        if let Some(publish_endpoint) = self.get_src_by_id(&webrtc_publish_id) {
            return Ok(Endpoint::WebRtcPublishEndpoint(publish_endpoint));
        }

        let webrtc_play_id = String::from(webrtc_publish_id).into();
        if let Some(play_endpoint) = self.get_sink_by_id(&webrtc_play_id) {
            return Ok(Endpoint::WebRtcPlayEndpoint(play_endpoint));
        }

        Err(MemberError::EndpointNotFound(
            self.get_fid_to_endpoint(webrtc_play_id.into()),
        ))
    }

    /// Downgrades strong [`Member`]'s pointer to weak [`WeakMember`] pointer.
    pub fn downgrade(&self) -> WeakMember {
        WeakMember(Rc::downgrade(&self.0))
    }

    /// Compares pointers. If both pointers point to the same address, then
    /// returns `true`.
    #[cfg(test)]
    pub fn ptr_eq(&self, another_member: &Self) -> bool {
        Rc::ptr_eq(&self.0, &another_member.0)
    }

    /// Returns [`CallbackUrl`] to which Medea should send `OnJoin` callback.
    pub fn get_on_join(&self) -> Option<CallbackUrl> {
        self.0.borrow().on_join.clone()
    }

    /// Returns [`CallbackUrl`] to which Medea should send `OnLeave` callback.
    pub fn get_on_leave(&self) -> Option<CallbackUrl> {
        self.0.borrow().on_leave.clone()
    }

    /// Sets all [`CallbackUrl`]'s from [`MemberSpec`].
    pub fn set_callback_urls(&self, spec: &MemberSpec) {
        self.0.borrow_mut().on_leave = spec.on_leave().clone();
        self.0.borrow_mut().on_join = spec.on_join().clone();
    }
}

/// Weak pointer to [`Member`].
#[derive(Clone, Debug)]
pub struct WeakMember(Weak<RefCell<MemberInner>>);

impl WeakMember {
    /// Upgrades weak pointer to strong pointer.
    ///
    /// This function will __panic__ if weak pointer was dropped.
    pub fn upgrade(&self) -> Member {
        Member(Weak::upgrade(&self.0).unwrap())
    }

    /// Safely upgrades to [`Member`].
    pub fn safe_upgrade(&self) -> Option<Member> {
        Weak::upgrade(&self.0).map(Member)
    }
}

/// Creates all empty [`Member`]s from [`RoomSpec`] and then
/// loads all related to this [`Member`]s sources and sinks endpoints.
///
/// Returns store of all [`Member`]s loaded from [`RoomSpec`].
pub fn parse_members(
    room_spec: &RoomSpec,
) -> Result<HashMap<MemberId, Member>, MembersLoadError> {
    let members_spec = room_spec.members().map_err(|e| {
        MembersLoadError::TryFromError(
            e,
            Fid::<ToRoom>::new(room_spec.id.clone()).into(),
        )
    })?;

    let members: HashMap<MemberId, Member> = members_spec
        .iter()
        .map(|(id, member)| {
            let new_member = Member::new(
                id.clone(),
                member.credentials().to_string(),
                room_spec.id.clone(),
            );
            (id.clone(), new_member)
        })
        .collect();

    for member in members.values() {
        member.load(room_spec, &members)?;
    }

    debug!(
        "Created ParticipantService with participants: {:?}.",
        members
            .iter()
            .map(|(id, p)| {
                format!(
                    "{{ id: {}, sinks: {:?}, srcs: {:?} }};",
                    id,
                    p.sinks()
                        .into_iter()
                        .map(|(id, _)| id.to_string())
                        .collect::<Vec<String>>(),
                    p.srcs()
                        .into_iter()
                        .map(|(id, _)| id.to_string())
                        .collect::<Vec<String>>()
                )
            })
            .collect::<Vec<String>>()
    );

    Ok(members)
}

impl Into<ElementProto> for Member {
    fn into(self) -> ElementProto {
        let mut element = ElementProto::new();
        let mut member = MemberProto::new();

        let mut member_pipeline = HashMap::new();
        for (id, play) in self.sinks() {
            member_pipeline.insert(id.to_string(), play.into());
        }
        for (id, publish) in self.srcs() {
            member_pipeline.insert(id.to_string(), publish.into());
        }
        member.set_pipeline(member_pipeline);

        member.set_id(self.id().to_string());
        member.set_credentials(self.credentials());
        if let Some(on_leave) = self.get_on_leave() {
            member.set_on_leave(on_leave.to_string());
        }
        if let Some(on_join) = self.get_on_join() {
            member.set_on_join(on_join.to_string());
        }

        element.set_member(member);

        element
    }
}

impl Into<RootElementProto> for Member {
    fn into(self) -> RootElementProto {
        let mut member_element: ElementProto = self.into();
        let member = member_element.take_member();

        let mut element = RootElementProto::new();
        element.set_member(member);

        element
    }
}

#[cfg(test)]
mod tests {
    use crate::api::control::{MemberId, RootElement};

    use super::*;

    const TEST_SPEC: &str = r#"
            kind: Room
            id: test-call
            spec:
              pipeline:
                caller:
                  kind: Member
                  credentials: test
                  spec:
                    pipeline:
                      publish:
                        kind: WebRtcPublishEndpoint
                        spec:
                          p2p: Always
                some-member:
                  kind: Member
                  credentials: test
                  spec:
                    pipeline:
                      publish:
                        kind: WebRtcPublishEndpoint
                        spec:
                          p2p: Always
                responder:
                  kind: Member
                  credentials: test
                  spec:
                    pipeline:
                      play:
                        kind: WebRtcPlayEndpoint
                        spec:
                          src: "local://test-call/caller/publish"
                      play2:
                        kind: WebRtcPlayEndpoint
                        spec:
                          src: "local://test-call/some-member/publish"
        "#;

    #[inline]
    fn id<T: From<String>>(s: &str) -> T {
        T::from(s.to_string())
    }

    fn get_test_store() -> HashMap<MemberId, Member> {
        let room_element: RootElement =
            serde_yaml::from_str(TEST_SPEC).unwrap();
        let room_spec = RoomSpec::try_from(&room_element).unwrap();
        parse_members(&room_spec).unwrap()
    }

    #[test]
    pub fn load_store() {
        let store = get_test_store();

        let caller = store.get(&id("caller")).unwrap();
        let responder = store.get(&id("responder")).unwrap();

        let caller_publish_endpoint =
            caller.get_src_by_id(&id("publish")).unwrap();
        let responder_play_endpoint =
            responder.get_sink_by_id(&id("play")).unwrap();

        let is_caller_has_responder_in_sinks = caller_publish_endpoint
            .sinks()
            .into_iter()
            .filter(|p| p.ptr_eq(&responder_play_endpoint))
            .count()
            == 1;
        assert!(is_caller_has_responder_in_sinks);

        assert!(responder_play_endpoint
            .src()
            .ptr_eq(&caller_publish_endpoint));

        let some_member = store.get(&id("some-member")).unwrap();
        assert!(some_member.sinks().is_empty());
        assert_eq!(some_member.srcs().len(), 1);

        let responder_play2_endpoint =
            responder.get_sink_by_id(&id("play2")).unwrap();
        let some_member_publisher =
            some_member.get_src_by_id(&id("publish")).unwrap();
        assert_eq!(some_member_publisher.sinks().len(), 1);
        let is_some_member_has_responder_in_sinks = some_member_publisher
            .sinks()
            .into_iter()
            .filter(|p| p.ptr_eq(&responder_play2_endpoint))
            .count()
            == 1;
        assert!(is_some_member_has_responder_in_sinks);
    }

    #[test]
    fn publisher_delete_all_their_players() {
        let store = get_test_store();

        let caller = store.get(&id("caller")).unwrap();
        let some_member = store.get(&id("some-member")).unwrap();
        let responder = store.get(&id("responder")).unwrap();

        caller.remove_src(&id("publish"));
        assert_eq!(responder.sinks().len(), 1);

        some_member.remove_src(&id("publish"));
        assert_eq!(responder.sinks().len(), 0);
    }

    #[test]
    fn player_delete_self_from_publisher_sink() {
        let store = get_test_store();

        let caller = store.get(&id("caller")).unwrap();
        let some_member = store.get(&id("some-member")).unwrap();
        let responder = store.get(&id("responder")).unwrap();

        let caller_publisher = caller.get_src_by_id(&id("publish")).unwrap();
        let some_member_publisher =
            some_member.get_src_by_id(&id("publish")).unwrap();

        responder.remove_sink(&id("play"));
        assert_eq!(caller_publisher.sinks().len(), 0);
        assert_eq!(some_member_publisher.sinks().len(), 1);

        responder.remove_sink(&id("play2"));
        assert_eq!(caller_publisher.sinks().len(), 0);
        assert_eq!(some_member_publisher.sinks().len(), 0);
    }
}

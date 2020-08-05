//! [`Member`] is member of [`Room`].
//!
//! [`Room`]: crate::signalling::room::Room

use std::{
    cell::RefCell,
    collections::HashMap,
    convert::TryFrom as _,
    rc::{Rc, Weak},
    time::Duration,
};

use derive_more::Display;
use failure::Fail;
use medea_client_api_proto::{MemberId, PeerId};
use medea_control_api_proto::grpc::api as proto;

use crate::{
    api::control::{
        callback::url::CallbackUrl,
        endpoints::WebRtcPlayEndpoint as WebRtcPlayEndpointSpec,
        refs::{Fid, StatefulFid, ToEndpoint, ToMember, ToRoom},
        EndpointId, MemberSpec, RoomId, RoomSpec, TryFromElementError,
        WebRtcPlayId, WebRtcPublishId,
    },
    conf::Rpc as RpcConf,
    log::prelude::*,
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

    /// URL to which `on_join` Control API callback will be sent.
    on_join: Option<CallbackUrl>,

    /// URL to which `on_leave` Control API callback will be sent.
    on_leave: Option<CallbackUrl>,

    /// Timeout of receiving heartbeat messages from the [`Member`] via Client
    /// API.
    ///
    /// Once reached, the [`Member`] is considered being idle.
    idle_timeout: Duration,

    /// Timeout of the [`Member`] reconnecting via Client API.
    ///
    /// Once reached, the [`Member`] is considered disconnected.
    reconnect_timeout: Duration,

    /// Interval of sending heartbeat `Ping`s to the [`Member`] via Client API.
    ping_interval: Duration,
}

impl Member {
    /// Creates new empty [`Member`].
    ///
    /// To fill this [`Member`], you need to call [`Member::load`]
    /// function.
    pub fn new(
        id: MemberId,
        credentials: String,
        room_id: RoomId,
        idle_timeout: Duration,
        reconnect_timeout: Duration,
        ping_interval: Duration,
    ) -> Self {
        Self(Rc::new(RefCell::new(MemberInner {
            id,
            srcs: HashMap::new(),
            sinks: HashMap::new(),
            credentials,
            room_id,
            on_leave: None,
            on_join: None,
            idle_timeout,
            reconnect_timeout,
            ping_interval,
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
                    spec_play_endpoint.force_relay,
                );

                self.insert_sink(new_play_endpoint.clone());

                publisher.add_sink(new_play_endpoint.downgrade());
            } else {
                let new_publish_id = spec_play_endpoint.src.endpoint_id.clone();
                let new_publish = WebRtcPublishEndpoint::new(
                    new_publish_id,
                    publisher_endpoint.p2p,
                    publisher_member.downgrade(),
                    publisher_endpoint.force_relay,
                    publisher_endpoint.audio_settings,
                    publisher_endpoint.video_settings,
                );

                let new_self_play = WebRtcPlayEndpoint::new(
                    spec_play_name,
                    spec_play_endpoint.src.clone(),
                    new_publish.downgrade(),
                    this_member.downgrade(),
                    spec_play_endpoint.force_relay,
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
                    e.force_relay,
                    e.audio_settings,
                    e.video_settings,
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

    /// Returns partner [`Member`]s of this [`Member`].
    pub fn partners(&self) -> Vec<Member> {
        let this = self.0.borrow();
        this.srcs
            .values()
            .flat_map(|src| src.sinks().into_iter().map(|s| s.owner()))
            .chain(this.sinks.values().map(|s| s.src().owner()))
            .map(|member| (member.id(), member))
            .collect::<HashMap<_, _>>()
            .into_iter()
            .map(|(_, member)| member)
            .collect()
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
            spec.force_relay,
        );

        src.add_sink(sink.downgrade());
        member.insert_sink(sink);
    }

    /// Lookups [`WebRtcPublishEndpoint`] and [`WebRtcPlayEndpoint`] at one
    /// moment by ID.
    ///
    /// # Errors
    ///
    /// Errors with [`MemberError::EndpointNotFound`] if no [`Endpoint`] with
    /// provided ID was found.
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

    /// Returns timeout of receiving heartbeat messages from the [`Member`] via
    /// Client API.
    ///
    /// Once reached, the [`Member`] is considered being idle.
    pub fn get_idle_timeout(&self) -> Duration {
        self.0.borrow().idle_timeout
    }

    /// Returns timeout of the [`Member`] reconnecting via Client API.
    ///
    /// Once reached, the [`Member`] is considered disconnected.
    pub fn get_reconnect_timeout(&self) -> Duration {
        self.0.borrow().reconnect_timeout
    }

    /// Returns interval of sending heartbeat `Ping`s to the [`Member`] via
    /// Client API.
    pub fn get_ping_interval(&self) -> Duration {
        self.0.borrow().ping_interval
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
///
/// # Errors
///
/// Errors with [`MembersLoadError::TryFromError`] if converting [`MemberSpec`]s
/// from [`RoomSpec`].
///
/// Errors with [`MembersLoadError`] if loading [`Member`] fails.
pub fn parse_members(
    room_spec: &RoomSpec,
    rpc_conf: RpcConf,
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
                member.idle_timeout().unwrap_or(rpc_conf.idle_timeout),
                member
                    .reconnect_timeout()
                    .unwrap_or(rpc_conf.reconnect_timeout),
                member.ping_interval().unwrap_or(rpc_conf.ping_interval),
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

impl Into<proto::Member> for Member {
    fn into(self) -> proto::Member {
        let member_pipeline = self
            .sinks()
            .into_iter()
            .map(|(id, play)| (id.to_string(), play.into()))
            .chain(
                self.srcs()
                    .into_iter()
                    .map(|(id, publish)| (id.to_string(), publish.into())),
            )
            .collect();

        proto::Member {
            id: self.id().to_string(),
            credentials: self.credentials(),
            on_leave: self
                .get_on_leave()
                .map(|c| c.to_string())
                .unwrap_or_default(),
            on_join: self
                .get_on_join()
                .map(|c| c.to_string())
                .unwrap_or_default(),
            reconnect_timeout: Some(self.get_reconnect_timeout().into()),
            idle_timeout: Some(self.get_idle_timeout().into()),
            ping_interval: Some(self.get_ping_interval().into()),
            pipeline: member_pipeline,
        }
    }
}

impl Into<proto::room::Element> for Member {
    fn into(self) -> proto::room::Element {
        proto::room::Element {
            el: Some(proto::room::element::El::Member(self.into())),
        }
    }
}

impl Into<proto::Element> for Member {
    fn into(self) -> proto::Element {
        proto::Element {
            el: Some(proto::element::El::Member(self.into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use medea_client_api_proto::MemberId;

    use crate::api::control::RootElement;

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
        parse_members(&room_spec, RpcConf::default()).unwrap()
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

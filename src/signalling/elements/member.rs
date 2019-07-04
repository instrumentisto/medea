//! [`Member`] is member of [`Room`] with [`RpcConnection`].

use std::{cell::RefCell, convert::TryFrom as _, rc::Rc};

use failure::Fail;
use hashbrown::HashMap;
use medea_client_api_proto::IceServer;

use crate::{
    api::control::{
        MemberId, SerdeMemberSpec, SerdeRoomSpec, TryFromElementError,
    },
    log::prelude::*,
    media::{IceUser, PeerId},
};

use super::endpoints::webrtc::{
    WebRtcPlayEndpoint, WebRtcPlayId, WebRtcPublishEndpoint, WebRtcPublishId,
};

/// Errors which may occur while loading [`Member`]s from [`RoomSpec`].
#[derive(Debug, Fail)]
pub enum MembersLoadError {
    /// Errors that can occur when we try transform some spec from [`Element`].
    #[fail(display = "TryFromElementError: {}", _0)]
    TryFromError(TryFromElementError),

    /// [`Member`] not found.
    #[fail(display = "Member with id '{}' not found.", _0)]
    MemberNotFound(MemberId),

    /// [`Endpoint`] not found.
    #[fail(display = "Endpoint with id '{}' not found.", _0)]
    EndpointNotFound(String),
}

impl From<TryFromElementError> for MembersLoadError {
    fn from(err: TryFromElementError) -> Self {
        MembersLoadError::TryFromError(err)
    }
}

/// [`Member`] is member of [`Room`] with [`RpcConnection`].
#[derive(Debug)]
pub struct Member(RefCell<MemberInner>);

#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
struct MemberInner {
    id: MemberId,

    /// All [`WebRtcPublishEndpoint`]s of this [`Member`].
    srcs: HashMap<WebRtcPublishId, Rc<WebRtcPublishEndpoint>>,

    /// All [`WebRtcPlayEndpoint`]s of this [`Member`].
    sinks: HashMap<WebRtcPlayId, Rc<WebRtcPlayEndpoint>>,

    /// Credentials for this [`Member`].
    credentials: String,

    /// [`IceUser`] of this [`Member`].
    ice_user: Option<IceUser>,
}

impl Member {
    /// Create new empty [`Member`].
    ///
    /// To fill this [`Member`], you need to call the [`Member::load`]
    /// function.
    fn new(id: MemberId, credentials: String) -> Self {
        Self(RefCell::new(MemberInner {
            id,
            srcs: HashMap::new(),
            sinks: HashMap::new(),
            credentials,
            ice_user: None,
        }))
    }

    /// Load all srcs and sinks of this [`Member`].
    fn load(
        &self,
        room_spec: &SerdeRoomSpec,
        store: &HashMap<MemberId, Rc<Self>>,
    ) -> Result<(), MembersLoadError> {
        let this_member_spec = SerdeMemberSpec::try_from(
            room_spec
                .pipeline
                .get(&self.id().0)
                .map_or(Err(MembersLoadError::MemberNotFound(self.id())), Ok)?,
        )?;

        let this_member = store
            .get(&self.id())
            .map_or(Err(MembersLoadError::MemberNotFound(self.id())), Ok)?;

        for (spec_play_name, spec_play_endpoint) in
            this_member_spec.play_endpoints()
        {
            let publisher_id =
                MemberId(spec_play_endpoint.src.member_id.to_string());
            let publisher_member = store.get(&publisher_id).map_or(
                Err(MembersLoadError::MemberNotFound(publisher_id)),
                Ok,
            )?;
            let publisher_spec = SerdeMemberSpec::try_from(
                room_spec
                    .pipeline
                    .get(&spec_play_endpoint.src.member_id.to_string())
                    .map_or(
                        Err(MembersLoadError::MemberNotFound(
                            spec_play_endpoint.src.member_id.clone(),
                        )),
                        Ok,
                    )?,
            )?;

            let publisher_endpoint = *publisher_spec
                .publish_endpoints()
                .get(&spec_play_endpoint.src.endpoint_id)
                .map_or(
                    Err(MembersLoadError::EndpointNotFound(
                        spec_play_endpoint.src.endpoint_id.clone().0, // TODO: tmp
                    )),
                    Ok,
                )?;

            if let Some(publisher) =
                publisher_member.get_src_by_id(&WebRtcPublishId(
                    spec_play_endpoint.src.endpoint_id.to_string(),
                ))
            {
                let new_play_endpoint_id =
                    WebRtcPlayId(spec_play_name.to_string());
                let new_play_endpoint = Rc::new(WebRtcPlayEndpoint::new(
                    new_play_endpoint_id.clone(),
                    spec_play_endpoint.src.clone(),
                    Rc::downgrade(&publisher),
                    Rc::downgrade(&this_member),
                ));

                self.insert_sink(Rc::clone(&new_play_endpoint));

                publisher.add_sink(Rc::downgrade(&new_play_endpoint));
            } else {
                let new_publish_id = WebRtcPublishId(
                    spec_play_endpoint.src.endpoint_id.to_string(),
                );
                let new_publish = Rc::new(WebRtcPublishEndpoint::new(
                    new_publish_id.clone(),
                    publisher_endpoint.p2p.clone(),
                    Vec::new(),
                    Rc::downgrade(&publisher_member),
                ));

                let new_self_play_id = WebRtcPlayId(spec_play_name.to_string());
                let new_self_play = Rc::new(WebRtcPlayEndpoint::new(
                    new_self_play_id.clone(),
                    spec_play_endpoint.src.clone(),
                    Rc::downgrade(&new_publish),
                    Rc::downgrade(&this_member),
                ));

                new_publish.add_sink(Rc::downgrade(&new_self_play));

                publisher_member.insert_src(new_publish);

                self.insert_sink(new_self_play);
            }
        }

        // This is necessary to create [`WebRtcPublishEndpoint`],
        // to which none [`WebRtcPlayEndpoint`] refers.
        this_member_spec.publish_endpoints().into_iter().for_each(
            |(name, e)| {
                let endpoint_id = WebRtcPublishId(name.clone().0); // TODO: tmp
                if self.srcs().get(&endpoint_id).is_none() {
                    self.insert_src(Rc::new(WebRtcPublishEndpoint::new(
                        endpoint_id,
                        e.p2p.clone(),
                        Vec::new(),
                        Rc::downgrade(&this_member),
                    )));
                }
            },
        );

        Ok(())
    }

    /// Notify [`Member`] that some [`Peer`]s removed.
    ///
    /// All [`PeerId`]s related to this [`Member`] will be removed.
    pub fn peers_removed(&self, peer_ids: &[PeerId]) {
        self.srcs()
            .into_iter()
            .for_each(|(_, p)| p.remove_peer_ids(peer_ids));

        self.sinks()
            .into_iter()
            .filter_map(|(_, p)| p.peer_id().map(|id| (id, p)))
            .filter(|(id, _)| peer_ids.contains(&id))
            .for_each(|(_, p)| p.reset());
    }

    /// Returns list of [`IceServer`] for this [`Member`].
    pub fn servers_list(&self) -> Option<Vec<IceServer>> {
        self.0.borrow().ice_user.as_ref().map(IceUser::servers_list)
    }

    /// Returns and set to `None` [`IceUser`] of this [`Member`].
    pub fn take_ice_user(&self) -> Option<IceUser> {
        self.0.borrow_mut().ice_user.take()
    }

    /// Replace and return [`IceUser`] of this [`Member`].
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

    /// Returns all publishers of this [`Member`].
    pub fn srcs(&self) -> HashMap<WebRtcPublishId, Rc<WebRtcPublishEndpoint>> {
        self.0.borrow().srcs.clone()
    }

    /// Returns all sinks endpoints of this [`Member`].
    pub fn sinks(&self) -> HashMap<WebRtcPlayId, Rc<WebRtcPlayEndpoint>> {
        self.0.borrow().sinks.clone()
    }

    /// Insert sink endpoint into this [`Member`].
    pub fn insert_sink(&self, endpoint: Rc<WebRtcPlayEndpoint>) {
        self.0.borrow_mut().sinks.insert(endpoint.id(), endpoint);
    }

    /// Insert source endpoint into this [`Member`].
    pub fn insert_src(&self, endpoint: Rc<WebRtcPublishEndpoint>) {
        self.0.borrow_mut().srcs.insert(endpoint.id(), endpoint);
    }

    /// Lookup [`WebRtcPublishEndpoint`] source endpoint by [`EndpointId`].
    pub fn get_src_by_id(
        &self,
        id: &WebRtcPublishId,
    ) -> Option<Rc<WebRtcPublishEndpoint>> {
        self.0.borrow().srcs.get(id).cloned()
    }

    /// Lookup [`WebRtcPlayEndpoint`] sink endpoint by [`EndpointId`].
    pub fn get_sink_by_id(
        &self,
        id: &WebRtcPlayId,
    ) -> Option<Rc<WebRtcPlayEndpoint>> {
        self.0.borrow().sinks.get(id).cloned()
    }

    /// Remove sink [`WebRtcPlayEndpoint`] from this [`Member`].
    pub fn remove_sink(&self, id: &WebRtcPlayId) {
        self.0.borrow_mut().sinks.remove(id);
    }

    /// Remove source [`WebRtcPublishEndpoint`] from this [`Member`].
    pub fn remove_src(&self, id: &WebRtcPublishId) {
        self.0.borrow_mut().srcs.remove(id);
    }
}

/// Creates all empty [`Member`] from [`RoomSpec`] and then
/// load all related to this [`Member`]s srcs and sinks endpoints.
///
/// Returns store of all [`Member`]s loaded from [`RoomSpec`].
pub fn parse_members(
    room_spec: &SerdeRoomSpec,
) -> Result<HashMap<MemberId, Rc<Member>>, MembersLoadError> {
    let members_spec = room_spec.members()?;
    let mut members = HashMap::new();

    for (id, member) in &members_spec {
        members.insert(
            id.clone(),
            Rc::new(Member::new(id.clone(), member.credentials().to_string())),
        );
    }

    for (_, member) in &members {
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

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use crate::api::control::{Element, MemberId};

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

    fn get_test_store() -> HashMap<MemberId, Rc<Member>> {
        let room_element: Element = serde_yaml::from_str(TEST_SPEC).unwrap();
        let room_spec = SerdeRoomSpec::try_from(&room_element).unwrap();
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
            .filter(|p| Rc::ptr_eq(p, &responder_play_endpoint))
            .count()
            == 1;
        assert!(is_caller_has_responder_in_sinks);

        assert!(Rc::ptr_eq(
            &responder_play_endpoint.src(),
            &caller_publish_endpoint
        ));

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
            .filter(|p| Rc::ptr_eq(p, &responder_play2_endpoint))
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

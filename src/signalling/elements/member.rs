//! [`Member`] is member of [`Room`] with [`RpcConnection`].

use std::{
    cell::RefCell, collections::HashMap as StdHashMap, convert::TryFrom as _,
    rc::Rc,
};

use failure::Fail;
use hashbrown::HashMap;
use medea_client_api_proto::IceServer;

use crate::{
    api::control::{
        endpoints::WebRtcPlayEndpoint as WebRtcPlayEndpointSpec,
        grpc::protos::control::{
            Member as MemberProto, Room_Element as ElementProto,
        },
        local_uri::LocalUri,
        MemberId, MemberSpec, RoomId, RoomSpec, TryFromElementError,
        WebRtcPlayId, WebRtcPublishId,
    },
    log::prelude::*,
    media::{IceUser, PeerId},
};

use super::endpoints::webrtc::{WebRtcPlayEndpoint, WebRtcPublishEndpoint};
use crate::api::error_codes::ErrorCode;

/// Errors which may occur while loading [`Member`]s from [`RoomSpec`].
#[derive(Debug, Fail)]
pub enum MembersLoadError {
    /// Errors that can occur when we try transform some spec from [`Element`].
    #[fail(display = "TryFromElementError: {}", _0)]
    TryFromError(TryFromElementError, LocalUri),

    /// [`Member`] not found.
    #[fail(display = "Member [id = {}] not found.", _0)]
    MemberNotFound(LocalUri),

    /// [`WebRtcPlayEndpoint`] not found.
    #[fail(
        display = "Play endpoint [id = {}] not found while loading spec,",
        _0
    )]
    PlayEndpointNotFound(LocalUri),

    /// [`WebRtcPublishEndpoint`] not found.
    #[fail(
        display = "Publish endpoint [id = {}] not found while loading spec.",
        _0
    )]
    PublishEndpointNotFound(LocalUri),
}

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Fail)]
pub enum MemberError {
    #[fail(display = "Publish endpoint [id = {}] not found.", _0)]
    PublishEndpointNotFound(LocalUri),

    #[fail(display = "Play endpoint [id = {}] not found.", _0)]
    PlayEndpointNotFound(LocalUri),
}

impl Into<ErrorCode> for MembersLoadError {
    fn into(self) -> ErrorCode {
        match self {
            MembersLoadError::TryFromError(e, id) => match e {
                TryFromElementError::NotEndpoint => {
                    ErrorCode::NotEndpointInSpec(id)
                }
                TryFromElementError::NotMember => {
                    ErrorCode::NotMemberInSpec(id)
                }
                TryFromElementError::NotRoom => ErrorCode::NotRoomInSpec(id),
            },
            MembersLoadError::MemberNotFound(id) => {
                ErrorCode::MemberNotFound(id)
            }
            MembersLoadError::PublishEndpointNotFound(id) => {
                ErrorCode::PublishEndpointNotFound(id)
            }
            MembersLoadError::PlayEndpointNotFound(id) => {
                ErrorCode::PlayEndpointNotFound(id)
            }
        }
    }
}

impl Into<ErrorCode> for MemberError {
    fn into(self) -> ErrorCode {
        match self {
            MemberError::PlayEndpointNotFound(id) => {
                ErrorCode::PlayEndpointNotFound(id)
            }
            MemberError::PublishEndpointNotFound(id) => {
                ErrorCode::PublishEndpointNotFound(id)
            }
        }
    }
}

/// [`Member`] is member of [`Room`] with [`RpcConnection`].
#[derive(Debug)]
pub struct Member(RefCell<MemberInner>);

#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
struct MemberInner {
    room_id: RoomId,

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
    // TODO: private
    pub fn new(id: MemberId, credentials: String, room_id: RoomId) -> Self {
        Self(RefCell::new(MemberInner {
            id,
            srcs: HashMap::new(),
            sinks: HashMap::new(),
            credentials,
            ice_user: None,
            room_id,
        }))
    }

    /// Lookup [`MemberSpec`] by ID from [`MemberSpec`].
    ///
    /// Returns [`MembersLoadError::MemberNotFound`] when member not found.
    /// Returns [`MembersLoadError::TryFromError`] when found element which is
    /// not [`MemberSpec`].
    fn get_member_from_room_spec(
        &self,
        room_spec: &RoomSpec,
        member_id: &MemberId,
    ) -> Result<MemberSpec, MembersLoadError> {
        let element = room_spec.pipeline.get(&member_id.0).map_or(
            Err(MembersLoadError::MemberNotFound(LocalUri::new(
                Some(self.room_id()),
                Some(member_id.clone()),
                None,
            ))),
            Ok,
        )?;

        MemberSpec::try_from(element).map_err(|e| {
            MembersLoadError::TryFromError(
                e,
                LocalUri::new(
                    Some(self.room_id()),
                    Some(member_id.clone()),
                    None,
                ),
            )
        })
    }

    /// Load all srcs and sinks of this [`Member`].
    fn load(
        &self,
        room_spec: &RoomSpec,
        store: &HashMap<MemberId, Rc<Self>>,
    ) -> Result<(), MembersLoadError> {
        let self_id = self.id();

        let this_member_spec =
            self.get_member_from_room_spec(room_spec, &self_id)?;

        let this_member = store.get(&self.id()).map_or(
            Err(MembersLoadError::MemberNotFound(self.get_local_uri())),
            Ok,
        )?;

        for (spec_play_name, spec_play_endpoint) in
            this_member_spec.play_endpoints()
        {
            let publisher_id =
                MemberId(spec_play_endpoint.src.member_id.to_string());
            let publisher_member = store.get(&publisher_id).map_or(
                Err(MembersLoadError::MemberNotFound(LocalUri::new(
                    Some(self.room_id()),
                    Some(publisher_id),
                    None,
                ))),
                Ok,
            )?;
            let publisher_spec = self.get_member_from_room_spec(
                room_spec,
                &spec_play_endpoint.src.member_id,
            )?;

            let publisher_endpoint = *publisher_spec
                .publish_endpoints()
                .get(&spec_play_endpoint.src.endpoint_id)
                .map_or(
                    Err(MembersLoadError::PublishEndpointNotFound(
                        publisher_member.get_local_uri_to_endpoint(
                            spec_play_endpoint.src.endpoint_id.to_string(),
                        ),
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
                let new_publish_id = &spec_play_endpoint.src.endpoint_id;
                let new_publish = Rc::new(WebRtcPublishEndpoint::new(
                    new_publish_id.clone(),
                    publisher_endpoint.p2p.clone(),
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
            |(endpoint_id, e)| {
                if self.srcs().get(&endpoint_id).is_none() {
                    self.insert_src(Rc::new(WebRtcPublishEndpoint::new(
                        endpoint_id,
                        e.p2p.clone(),
                        Rc::downgrade(&this_member),
                    )));
                }
            },
        );

        Ok(())
    }

    /// Return [`LocalUri`] to this [`Member`].
    fn get_local_uri(&self) -> LocalUri {
        LocalUri::new(Some(self.room_id()), Some(self.id()), None)
    }

    /// Return [`LocalUri`] to some endpoint from this [`Member`].
    ///
    /// __Note__ this function don't check presence of `Endpoint` in this
    /// [`Member`].
    pub fn get_local_uri_to_endpoint(&self, endpoint_id: String) -> LocalUri {
        LocalUri::new(Some(self.room_id()), Some(self.id()), Some(endpoint_id))
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

    /// Lookup [`WebRtcPublishEndpoint`] source endpoint by [`WebRtcPublishId`].
    pub fn get_src_by_id(
        &self,
        id: &WebRtcPublishId,
    ) -> Option<Rc<WebRtcPublishEndpoint>> {
        self.0.borrow().srcs.get(id).cloned()
    }

    /// Lookup [`WebRtcPublishEndpoint`] source endpoint by [`WebRtcPublishId`].
    ///
    /// Returns [`MeberError::PublishEndpointNotFound`] when
    /// [`WebRtcPublishEndpoint`] not found.
    pub fn get_src(
        &self,
        id: &WebRtcPublishId,
    ) -> Result<Rc<WebRtcPublishEndpoint>, MemberError> {
        self.0.borrow().srcs.get(id).cloned().map_or_else(
            || {
                Err(MemberError::PublishEndpointNotFound(
                    self.get_local_uri_to_endpoint(id.to_string()),
                ))
            },
            Ok,
        )
    }

    /// Lookup [`WebRtcPlayEndpoint`] sink endpoint by [`EndpointId`].
    pub fn get_sink_by_id(
        &self,
        id: &WebRtcPlayId,
    ) -> Option<Rc<WebRtcPlayEndpoint>> {
        self.0.borrow().sinks.get(id).cloned()
    }

    /// Lookup [`WebRtcPlayEndpoint`] sink endpoint by [`EndpointId`].
    ///
    /// Returns [`MemberError::PlayEndpointNotFound`] when
    /// [`WebRtcPlayEndpoint`] not found.
    pub fn get_sink(
        &self,
        id: &WebRtcPlayId,
    ) -> Result<Rc<WebRtcPlayEndpoint>, MemberError> {
        self.0.borrow().sinks.get(id).cloned().map_or_else(
            || {
                Err(MemberError::PlayEndpointNotFound(
                    self.get_local_uri_to_endpoint(id.to_string()),
                ))
            },
            Ok,
        )
    }

    /// Remove sink [`WebRtcPlayEndpoint`] from this [`Member`].
    pub fn remove_sink(&self, id: &WebRtcPlayId) {
        self.0.borrow_mut().sinks.remove(id);
    }

    /// Remove source [`WebRtcPublishEndpoint`] from this [`Member`].
    pub fn remove_src(&self, id: &WebRtcPublishId) {
        self.0.borrow_mut().srcs.remove(id);
    }

    /// Take sink from [`Member`]'s `sinks`.
    pub fn take_sink(
        &self,
        id: &WebRtcPlayId,
    ) -> Option<Rc<WebRtcPlayEndpoint>> {
        self.0.borrow_mut().sinks.remove(id)
    }

    /// Take src from [`Member`]'s `srsc`.
    pub fn take_src(
        &self,
        id: &WebRtcPublishId,
    ) -> Option<Rc<WebRtcPublishEndpoint>> {
        self.0.borrow_mut().srcs.remove(id)
    }

    pub fn room_id(&self) -> RoomId {
        self.0.borrow().room_id.clone()
    }

    /// Create new [`WebRtcPlayEndpoint`] based on provided
    /// [`WebRtcPlayEndpointSpec`].
    ///
    /// This function will add created [`WebRtcPlayEndpoint`] to src's
    /// [`WebRtcPublishEndpoint`] and to provided [`Member`].
    pub fn create_sink(
        member: &Rc<Self>,
        id: WebRtcPlayId,
        spec: WebRtcPlayEndpointSpec,
    ) {
        let src = member.get_src_by_id(&spec.src.endpoint_id).unwrap();

        let sink = Rc::new(WebRtcPlayEndpoint::new(
            id,
            spec.src,
            Rc::downgrade(&src),
            Rc::downgrade(&member),
        ));

        src.add_sink(Rc::downgrade(&sink));
        member.insert_sink(sink);
    }
}

/// Creates all empty [`Member`] from [`RoomSpec`] and then
/// load all related to this [`Member`]s srcs and sinks endpoints.
///
/// Returns store of all [`Member`]s loaded from [`RoomSpec`].
pub fn parse_members(
    room_spec: &RoomSpec,
) -> Result<HashMap<MemberId, Rc<Member>>, MembersLoadError> {
    let members_spec = match room_spec.members() {
        Ok(o) => o,
        Err(e) => {
            return Err(MembersLoadError::TryFromError(
                e,
                LocalUri::new(Some(room_spec.id.clone()), None, None),
            ))
        }
    };
    let mut members = HashMap::new();

    for (id, member) in &members_spec {
        members.insert(
            id.clone(),
            Rc::new(Member::new(
                id.clone(),
                member.credentials().to_string(),
                room_spec.id.clone(),
            )),
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

impl Into<ElementProto> for Rc<Member> {
    fn into(self) -> ElementProto {
        let mut element = ElementProto::new();
        let mut member = MemberProto::new();

        let mut member_pipeline = StdHashMap::new();
        for (id, play) in self.sinks() {
            let local_uri = LocalUri {
                room_id: Some(self.room_id()),
                member_id: Some(self.id()),
                endpoint_id: Some(id.to_string()),
            };
            member_pipeline.insert(local_uri.to_string(), play.into());
        }
        for (id, publish) in self.srcs() {
            let local_uri = LocalUri {
                room_id: Some(self.room_id()),
                member_id: Some(self.id()),
                endpoint_id: Some(id.to_string()),
            };

            member_pipeline.insert(local_uri.to_string(), publish.into());
        }
        member.set_pipeline(member_pipeline);

        member.set_credentials(self.credentials());

        element.set_member(member);

        element
    }
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

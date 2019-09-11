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

use crate::{
    api::control::{MemberId, MemberSpec, RoomSpec, TryFromElementError},
    log::prelude::*,
    media::IceUser,
};

use super::endpoints::webrtc::{
    WebRtcPlayEndpoint, WebRtcPlayId, WebRtcPublishEndpoint, WebRtcPublishId,
};

/// Errors which may occur while loading [`Member`]s from [`RoomSpec`].
#[derive(Debug, Display, Fail)]
pub enum MembersLoadError {
    /// Errors that can occur when we try transform some spec from `Element`.
    #[display(fmt = "TryFromElementError: {}", _0)]
    TryFromError(TryFromElementError),

    /// [`Member`] not found.
    #[display(fmt = "Member with id '{}' not found.", _0)]
    MemberNotFound(MemberId),

    /// [`Endpoint`] not found.
    ///
    /// [`Endpoint`]: crate::api::control::endpoint::Endpoint
    #[display(fmt = "Endpoint with id '{}' not found.", _0)]
    EndpointNotFound(String),
}

impl From<TryFromElementError> for MembersLoadError {
    fn from(err: TryFromElementError) -> Self {
        Self::TryFromError(err)
    }
}

/// [`Member`] is member of [`Room`].
///
/// [`Room`]: crate::signalling::room::Room
#[derive(Clone, Debug)]
pub struct Member(Rc<RefCell<MemberInner>>);

#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
struct MemberInner {
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
}

impl Member {
    /// Create new empty [`Member`].
    ///
    /// To fill this [`Member`], you need to call [`Member::load`]
    /// function.
    fn new(id: MemberId, credentials: String) -> Self {
        Self(Rc::new(RefCell::new(MemberInner {
            id,
            srcs: HashMap::new(),
            sinks: HashMap::new(),
            credentials,
            ice_user: None,
        })))
    }

    /// Loads all sources and sinks of this [`Member`].
    fn load(
        &self,
        room_spec: &RoomSpec,
        store: &HashMap<MemberId, Self>,
    ) -> Result<(), MembersLoadError> {
        let this_member_spec = MemberSpec::try_from(
            room_spec
                .pipeline
                .get(&self.id().0)
                .ok_or_else(|| MembersLoadError::MemberNotFound(self.id()))?,
        )?;

        let this_member = store
            .get(&self.id())
            .ok_or_else(|| MembersLoadError::MemberNotFound(self.id()))?;

        for (spec_play_name, spec_play_endpoint) in
            this_member_spec.play_endpoints()
        {
            let publisher_id =
                MemberId(spec_play_endpoint.src.member_id.to_string());
            let publisher_member =
                store.get(&publisher_id).ok_or_else(|| {
                    MembersLoadError::MemberNotFound(publisher_id)
                })?;
            let publisher_spec = MemberSpec::try_from(
                room_spec
                    .pipeline
                    .get(&spec_play_endpoint.src.member_id.to_string())
                    .ok_or_else(|| {
                        MembersLoadError::MemberNotFound(
                            spec_play_endpoint.src.member_id.clone(),
                        )
                    })?,
            )?;

            let publisher_endpoint = publisher_spec
                .get_publish_endpoint_by_id(&spec_play_endpoint.src.endpoint_id)
                .ok_or_else(|| {
                    MembersLoadError::EndpointNotFound(
                        spec_play_endpoint.src.endpoint_id.clone(),
                    )
                })?;

            if let Some(publisher) =
                publisher_member.get_src_by_id(&WebRtcPublishId(
                    spec_play_endpoint.src.endpoint_id.to_string(),
                ))
            {
                let new_play_endpoint_id =
                    WebRtcPlayId(spec_play_name.to_string());
                let new_play_endpoint = WebRtcPlayEndpoint::new(
                    new_play_endpoint_id,
                    spec_play_endpoint.src.clone(),
                    publisher.downgrade(),
                    this_member.downgrade(),
                );

                self.insert_sink(new_play_endpoint.clone());

                publisher.add_sink(new_play_endpoint.downgrade());
            } else {
                let new_publish_id = WebRtcPublishId(
                    spec_play_endpoint.src.endpoint_id.to_string(),
                );
                let new_publish = WebRtcPublishEndpoint::new(
                    new_publish_id,
                    publisher_endpoint.p2p.clone(),
                    Vec::new(),
                    publisher_member.downgrade(),
                );

                let new_self_play_id = WebRtcPlayId(spec_play_name.to_string());
                let new_self_play = WebRtcPlayEndpoint::new(
                    new_self_play_id,
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
        this_member_spec.publish_endpoints().for_each(|(name, e)| {
            let endpoint_id = WebRtcPublishId(name.clone());
            if self.srcs().get(&endpoint_id).is_none() {
                self.insert_src(WebRtcPublishEndpoint::new(
                    endpoint_id,
                    e.p2p.clone(),
                    Vec::new(),
                    this_member.downgrade(),
                ));
            }
        });

        Ok(())
    }

    /// Notify [`Member`] that some [`Peer`]s removed.
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

    /// Downgrades strong [`Member`]'s pointer to weak [`WeakMember`] pointer.
    pub fn downgrade(&self) -> WeakMember {
        WeakMember(Rc::downgrade(&self.0))
    }

    /// Compares pointers. If both pointers point to the same address, then
    /// returns true.
    #[cfg(test)]
    pub fn ptr_eq(&self, another_member: &Self) -> bool {
        Rc::ptr_eq(&self.0, &another_member.0)
    }
}

/// Weak pointer to [`Member`].
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug)]
pub struct WeakMember(Weak<RefCell<MemberInner>>);

impl WeakMember {
    /// Upgrade weak pointer to strong pointer.
    ///
    /// This function will __panic__ if weak pointer was dropped.
    pub fn upgrade(&self) -> Member {
        Member(Weak::upgrade(&self.0).unwrap())
    }

    /// Safe upgrade to [`Member`].
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
    let members_spec = room_spec.members()?;

    let members: HashMap<MemberId, Member> = members_spec
        .iter()
        .map(|(id, member)| {
            let new_member =
                Member::new(id.clone(), member.credentials().to_string());
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

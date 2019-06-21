//! [`Participant`] is member of [`Room`] with [`RpcConnection`].

use std::{
    convert::TryFrom as _,
    sync::{Arc, Mutex},
};

use failure::Fail;
use hashbrown::HashMap;
use medea_client_api_proto::IceServer;

use crate::{
    api::control::{MemberId, MemberSpec, RoomSpec, TryFromElementError},
    media::{IceUser, PeerId},
};

use super::endpoint::{
    Id as EndpointId, WebRtcPlayEndpoint, WebRtcPublishEndpoint,
};

/// Errors which may occur while loading [`Participant`]s from [`RoomSpec`].
#[derive(Debug, Fail)]
pub enum ParticipantsLoadError {
    /// Errors that can occur when we try transform some spec from [`Element`].
    #[fail(display = "TryFromElementError: {}", _0)]
    TryFromError(TryFromElementError),

    /// [`Participant`] not found.
    #[fail(display = "Member with id '{}' not found.", _0)]
    MemberNotFound(MemberId),

    /// [`Endpoint`] not found.
    #[fail(display = "Endpoint with id '{}' not found.", _0)]
    EndpointNotFound(String),
}

impl From<TryFromElementError> for ParticipantsLoadError {
    fn from(err: TryFromElementError) -> Self {
        ParticipantsLoadError::TryFromError(err)
    }
}

/// [`Participant`] is member of [`Room`] with [`RpcConnection`].
#[derive(Debug)]
pub struct Participant(Mutex<ParticipantInner>);

#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
struct ParticipantInner {
    id: MemberId,

    /// All [`WebRtcPublishEndpoint`]s of this [`Participant`].
    publishers: HashMap<EndpointId, Arc<WebRtcPublishEndpoint>>,

    /// All [`WebRtcPlayEndpoint`]s of this [`Participant`].
    receivers: HashMap<EndpointId, Arc<WebRtcPlayEndpoint>>,

    /// Credentials for this [`Participant`].
    credentials: String,

    /// [`IceUser`] of this [`Participant`].
    ice_user: Option<IceUser>,
}

impl Participant {
    /// Create new empty [`Participant`].
    ///
    /// To fill this [`Participant`], you need to call the [`Participant::load`]
    /// function.
    fn new(id: MemberId, credentials: String) -> Self {
        Self(Mutex::new(ParticipantInner {
            id,
            publishers: HashMap::new(),
            receivers: HashMap::new(),
            credentials,
            ice_user: None,
        }))
    }

    /// Notify [`Participant`] that some [`Peer`]s removed.
    ///
    /// All [`PeerId`]s related to this [`Participant`] will be removed.
    pub fn peers_removed(&self, peer_ids: &[PeerId]) {
        self.publishers()
            .into_iter()
            .for_each(|(_, p)| p.remove_peer_ids(peer_ids));

        self.receivers()
            .into_iter()
            .filter_map(|(_, p)| p.peer_id().map(|id| (id, p)))
            .filter(|(id, _)| peer_ids.contains(&id))
            .for_each(|(_, p)| p.reset());
    }

    /// Returns list of [`IceServer`] for this [`Participant`].
    pub fn servers_list(&self) -> Option<Vec<IceServer>> {
        self.0
            .lock()
            .unwrap()
            .ice_user
            .as_ref()
            .map(IceUser::servers_list)
    }

    /// Returns and set to `None` [`IceUser`] of this [`Participant`].
    pub fn take_ice_user(&self) -> Option<IceUser> {
        self.0.lock().unwrap().ice_user.take()
    }

    /// Replace and return [`IceUser`] of this [`Participant`].
    pub fn replace_ice_user(&self, new_ice_user: IceUser) -> Option<IceUser> {
        self.0.lock().unwrap().ice_user.replace(new_ice_user)
    }

    /// Returns [`MemberId`] of this [`Participant`].
    pub fn id(&self) -> MemberId {
        self.0.lock().unwrap().id.clone()
    }

    /// Returns credentials of this [`Participant`].
    pub fn credentials(&self) -> String {
        self.0.lock().unwrap().credentials.clone()
    }

    /// Creates all empty [`Participant`] from [`RoomSpec`] and then
    /// load all related to this [`Participant`]s receivers and publishers.
    ///
    /// Returns store of all [`Participant`]s loaded from [`RoomSpec`].
    pub fn load_store(
        room_spec: &RoomSpec,
    ) -> Result<HashMap<MemberId, Arc<Self>>, ParticipantsLoadError> {
        let members = room_spec.members()?;
        let mut participants = HashMap::new();

        for (id, member) in &members {
            participants.insert(
                id.clone(),
                Arc::new(Self::new(
                    id.clone(),
                    member.credentials().to_string(),
                )),
            );
        }

        for (_, participant) in &participants {
            participant.load(room_spec, &participants)?;
        }

        Ok(participants)
    }

    /// Returns all publishers of this [`Participant`].
    pub fn publishers(
        &self,
    ) -> HashMap<EndpointId, Arc<WebRtcPublishEndpoint>> {
        self.0.lock().unwrap().publishers.clone()
    }

    /// Returns all receivers of this [`Participant`].
    pub fn receivers(&self) -> HashMap<EndpointId, Arc<WebRtcPlayEndpoint>> {
        self.0.lock().unwrap().receivers.clone()
    }

    /// Load all publishers and receivers of this [`Participant`].
    fn load(
        &self,
        room_spec: &RoomSpec,
        store: &HashMap<MemberId, Arc<Self>>,
    ) -> Result<(), ParticipantsLoadError> {
        let this_member_spec = MemberSpec::try_from(
            room_spec.pipeline.get(&self.id().0).map_or(
                Err(ParticipantsLoadError::MemberNotFound(self.id())),
                Ok,
            )?,
        )?;

        let this_member = store.get(&self.id()).map_or(
            Err(ParticipantsLoadError::MemberNotFound(self.id())),
            Ok,
        )?;

        for (spec_play_name, spec_play_endpoint) in
            this_member_spec.play_endpoints()
        {
            let publisher_id =
                MemberId(spec_play_endpoint.src.member_id.to_string());
            let publisher_participant = store.get(&publisher_id).map_or(
                Err(ParticipantsLoadError::MemberNotFound(publisher_id)),
                Ok,
            )?;
            let publisher_spec = MemberSpec::try_from(
                room_spec
                    .pipeline
                    .get(&spec_play_endpoint.src.member_id.to_string())
                    .map_or(
                        Err(ParticipantsLoadError::MemberNotFound(
                            spec_play_endpoint.src.member_id.clone(),
                        )),
                        Ok,
                    )?,
            )?;

            let publisher_endpoint = *publisher_spec
                .publish_endpoints()
                .get(&spec_play_endpoint.src.endpoint_id)
                .map_or(
                    Err(ParticipantsLoadError::EndpointNotFound(
                        spec_play_endpoint.src.endpoint_id.clone(),
                    )),
                    Ok,
                )?;

            if let Some(publisher) = publisher_participant.get_publisher_by_id(
                &EndpointId(spec_play_endpoint.src.endpoint_id.to_string()),
            ) {
                let new_play_endpoint_id =
                    EndpointId(spec_play_name.to_string());
                let new_play_endpoint = Arc::new(WebRtcPlayEndpoint::new(
                    new_play_endpoint_id.clone(),
                    spec_play_endpoint.src.clone(),
                    Arc::downgrade(&publisher),
                    Arc::downgrade(&this_member),
                ));

                self.insert_receiver(Arc::clone(&new_play_endpoint));

                publisher.add_receiver(Arc::downgrade(&new_play_endpoint));
            } else {
                let new_publish_id =
                    EndpointId(spec_play_endpoint.src.endpoint_id.to_string());
                let new_publish = Arc::new(WebRtcPublishEndpoint::new(
                    new_publish_id.clone(),
                    publisher_endpoint.p2p.clone(),
                    Vec::new(),
                    Arc::downgrade(&publisher_participant),
                ));

                let new_self_play_id = EndpointId(spec_play_name.to_string());
                let new_self_play = Arc::new(WebRtcPlayEndpoint::new(
                    new_self_play_id.clone(),
                    spec_play_endpoint.src.clone(),
                    Arc::downgrade(&new_publish),
                    Arc::downgrade(&this_member),
                ));

                new_publish.add_receiver(Arc::downgrade(&new_self_play));

                publisher_participant.insert_publisher(new_publish);

                self.insert_receiver(new_self_play);
            }
        }

        // This is necessary to create [`WebRtcPublishEndpoint`],
        // to which none [`WebRtcPlayEndpoint`] refers.
        this_member_spec.publish_endpoints().into_iter().for_each(
            |(name, e)| {
                let endpoint_id = EndpointId(name.clone());
                if self.publishers().get(&endpoint_id).is_none() {
                    self.insert_publisher(Arc::new(
                        WebRtcPublishEndpoint::new(
                            endpoint_id,
                            e.p2p.clone(),
                            Vec::new(),
                            Arc::downgrade(&this_member),
                        ),
                    ));
                }
            },
        );

        Ok(())
    }

    /// Insert receiver into this [`Participant`].
    pub fn insert_receiver(&self, endpoint: Arc<WebRtcPlayEndpoint>) {
        self.0
            .lock()
            .unwrap()
            .receivers
            .insert(endpoint.id(), endpoint);
    }

    /// Insert publisher into this [`Participant`].
    pub fn insert_publisher(&self, endpoint: Arc<WebRtcPublishEndpoint>) {
        self.0
            .lock()
            .unwrap()
            .publishers
            .insert(endpoint.id(), endpoint);
    }

    /// Lookup [`WebRtcPublishEndpoint`] publisher by [`EndpointId`].
    pub fn get_publisher_by_id(
        &self,
        id: &EndpointId,
    ) -> Option<Arc<WebRtcPublishEndpoint>> {
        self.0.lock().unwrap().publishers.get(id).cloned()
    }

    /// Lookup [`WebRtcPlayEndpoint`] receiver by [`EndpointId`].
    pub fn get_receiver_by_id(
        &self,
        id: &EndpointId,
    ) -> Option<Arc<WebRtcPlayEndpoint>> {
        self.0.lock().unwrap().receivers.get(id).cloned()
    }

    /// Remove receiver [`WebRtcPlayEndpoint`] from this [`Participant`].
    pub fn remove_receiver(&self, id: &EndpointId) {
        self.0.lock().unwrap().receivers.remove(id);
    }

    /// Remove receiver [`WebRtcPublishEndpoint`] from this [`Participant`].
    pub fn remove_publisher(&self, id: &EndpointId) {
        self.0.lock().unwrap().publishers.remove(id);
    }
}

#[cfg(test)]
mod participant_loading_tests {
    use std::sync::Arc;

    use crate::api::control::Element;

    use super::*;

    #[test]
    pub fn load_store() {
        let spec = r#"
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
        let room_element: Element = serde_yaml::from_str(&spec).unwrap();
        let room_spec = RoomSpec::try_from(&room_element).unwrap();
        let store = Participant::load_store(&room_spec).unwrap();

        let caller = store.get(&MemberId("caller".to_string())).unwrap();
        let responder = store.get(&MemberId("responder".to_string())).unwrap();

        let caller_publish_endpoint = caller
            .get_publisher_by_id(&EndpointId("publish".to_string()))
            .unwrap();
        let responder_play_endpoint = responder
            .get_receiver_by_id(&EndpointId("play".to_string()))
            .unwrap();

        let is_caller_has_responder_in_receivers = caller_publish_endpoint
            .receivers()
            .into_iter()
            .map(|p| p.upgrade().unwrap())
            .filter(|p| Arc::ptr_eq(p, &responder_play_endpoint))
            .count()
            == 1;
        assert!(is_caller_has_responder_in_receivers);

        assert!(Arc::ptr_eq(
            &responder_play_endpoint.publisher().upgrade().unwrap(),
            &caller_publish_endpoint
        ));

        let some_participant =
            store.get(&MemberId("some-member".to_string())).unwrap();
        assert!(some_participant.receivers().is_empty());
        assert_eq!(some_participant.publishers().len(), 1);

        let responder_play2_endpoint = responder
            .get_receiver_by_id(&EndpointId("play2".to_string()))
            .unwrap();
        let some_participant_publisher = some_participant
            .get_publisher_by_id(&EndpointId("publish".to_string()))
            .unwrap();
        assert_eq!(some_participant_publisher.receivers().len(), 1);
        let is_some_participant_has_responder_in_receivers =
            some_participant_publisher
                .receivers()
                .into_iter()
                .map(|p| p.upgrade().unwrap())
                .filter(|p| Arc::ptr_eq(p, &responder_play2_endpoint))
                .count()
                == 1;
        assert!(is_some_participant_has_responder_in_receivers);
    }
}

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
    media::IceUser,
    media::PeerId,
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
    publishers: HashMap<EndpointId, WebRtcPublishEndpoint>,

    /// All [`WebRtcPlayEndpoint`]s of this [`Participant`].
    receivers: HashMap<EndpointId, WebRtcPlayEndpoint>,

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

    /// Remove publisher [`WebRtcPublishEndpoint`] from this [`Participant`].
    pub fn remove_publisher(&self, id: &EndpointId) {
        self.0.lock().unwrap().publishers.remove(id);
    }

    /// Remove receiver [`WebRtcPlayEndpoint`] from this [`Participant`].
    pub fn remove_receiver(&self, id: &EndpointId) {
        self.0.lock().unwrap().receivers.remove(id);
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
    pub fn publishers(&self) -> HashMap<EndpointId, WebRtcPublishEndpoint> {
        self.0.lock().unwrap().publishers.clone()
    }

    /// Returns all receivers of this [`Participant`].
    pub fn receivers(&self) -> HashMap<EndpointId, WebRtcPlayEndpoint> {
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
                let new_play_endpoint = WebRtcPlayEndpoint::new(
                    spec_play_endpoint.src.clone(),
                    publisher.clone(),
                    Arc::downgrade(&this_member),
                    new_play_endpoint_id.clone(),
                );

                self.insert_receiver(
                    EndpointId(spec_play_name.to_string()),
                    new_play_endpoint.clone(),
                );

                publisher.add_receiver(new_play_endpoint.clone());
            } else {
                let new_publish_endpoint_id =
                    EndpointId(spec_play_endpoint.src.endpoint_id.to_string());
                let new_publish = WebRtcPublishEndpoint::new(
                    publisher_endpoint.p2p.clone(),
                    Vec::new(),
                    Arc::downgrade(&publisher_participant),
                    new_publish_endpoint_id.clone(),
                );

                let new_self_play_endpoint_id =
                    EndpointId(spec_play_name.to_string());
                let new_self_play = WebRtcPlayEndpoint::new(
                    spec_play_endpoint.src.clone(),
                    new_publish.clone(),
                    Arc::downgrade(&this_member),
                    new_self_play_endpoint_id.clone(),
                );

                new_publish.add_receiver(new_self_play.clone());

                publisher_participant
                    .insert_publisher(new_publish_endpoint_id, new_publish);

                self.insert_receiver(new_self_play_endpoint_id, new_self_play);
            }
        }

        // This is necessary to create [`WebRtcPublishEndpoint`],
        // to which none [`WebRtcPlayEndpoint`] refers.
        this_member_spec.publish_endpoints().into_iter().for_each(
            |(name, e)| {
                let endpoint_id = EndpointId(name.clone());
                if self.publishers().get(&endpoint_id).is_none() {
                    self.insert_publisher(
                        endpoint_id.clone(),
                        WebRtcPublishEndpoint::new(
                            e.p2p.clone(),
                            Vec::new(),
                            Arc::downgrade(&this_member),
                            endpoint_id,
                        ),
                    );
                }
            },
        );

        Ok(())
    }

    /// Insert receiver into this [`Participant`].
    pub fn insert_receiver(
        &self,
        id: EndpointId,
        endpoint: WebRtcPlayEndpoint,
    ) {
        self.0.lock().unwrap().receivers.insert(id, endpoint);
    }

    /// Insert publisher into this [`Participant`].
    pub fn insert_publisher(
        &self,
        id: EndpointId,
        endpoint: WebRtcPublishEndpoint,
    ) {
        self.0.lock().unwrap().publishers.insert(id, endpoint);
    }

    /// Lookup [`WebRtcPublishEndpoint`] publisher by [`EndpointId`].
    pub fn get_publisher_by_id(
        &self,
        id: &EndpointId,
    ) -> Option<WebRtcPublishEndpoint> {
        self.0.lock().unwrap().publishers.get(id).cloned()
    }

    /// Lookup [`WebRtcPlayEndpoint`] receiver by [`EndpointId`].
    pub fn get_receiver_by_id(
        &self,
        id: &EndpointId,
    ) -> Option<WebRtcPlayEndpoint> {
        self.0.lock().unwrap().receivers.get(id).cloned()
    }
}

#[cfg(test)]
mod participant_loading_tests {
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
            .filter(|p| p.id() == responder_play_endpoint.id())
            .count()
            == 1;
        assert!(is_caller_has_responder_in_receivers);

        assert_eq!(
            responder_play_endpoint.publisher().id(),
            caller_publish_endpoint.id()
        );

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
                .filter(|p| p.id() == responder_play2_endpoint.id())
                .count()
                == 1;
        assert!(is_some_participant_has_responder_in_receivers);
    }
}

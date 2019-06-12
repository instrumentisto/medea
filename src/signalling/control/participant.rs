//! [`Participant`] is member of [`Room`] with [`RpcConnection`].

use std::{
    cell::RefCell,
    convert::TryFrom as _,
    sync::{Arc, Mutex},
};

use failure::Fail;
use hashbrown::HashMap;

use crate::{
    api::control::{MemberId, MemberSpec, RoomSpec, TryFromElementError},
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
pub struct Participant(Mutex<RefCell<ParticipantInner>>);

#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
struct ParticipantInner {
    id: MemberId,
    publishers: HashMap<EndpointId, Arc<WebRtcPublishEndpoint>>,
    receivers: HashMap<EndpointId, Arc<WebRtcPlayEndpoint>>,
    credentials: String,
}

impl Participant {
    /// Create new empty [`Participant`].
    fn new(id: MemberId, credentials: String) -> Self {
        Self(Mutex::new(RefCell::new(ParticipantInner {
            id,
            publishers: HashMap::new(),
            receivers: HashMap::new(),
            credentials,
        })))
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

    /// Returns [`MemberId`] of this [`Participant`].
    pub fn id(&self) -> MemberId {
        self.0.lock().unwrap().borrow().id.clone()
    }

    /// Returns credentials of this [`Participant`].
    pub fn credentials(&self) -> String {
        self.0.lock().unwrap().borrow().credentials.clone()
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
        self.0.lock().unwrap().borrow().publishers.clone()
    }

    /// Returns all receivers of this [`Participant`].
    pub fn receivers(&self) -> HashMap<EndpointId, Arc<WebRtcPlayEndpoint>> {
        self.0.lock().unwrap().borrow().receivers.clone()
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
                let new_play_endpoint = Arc::new(WebRtcPlayEndpoint::new(
                    spec_play_endpoint.src.clone(),
                    Arc::downgrade(&publisher),
                    Arc::downgrade(&this_member),
                ));

                self.insert_receiver(
                    EndpointId(spec_play_name.to_string()),
                    Arc::clone(&new_play_endpoint),
                );

                publisher.add_receiver(Arc::downgrade(&new_play_endpoint));
            } else {
                let new_publish = Arc::new(WebRtcPublishEndpoint::new(
                    publisher_endpoint.p2p.clone(),
                    Vec::new(),
                    Arc::downgrade(&publisher_participant),
                ));

                let new_self_play = Arc::new(WebRtcPlayEndpoint::new(
                    spec_play_endpoint.src.clone(),
                    Arc::downgrade(&new_publish),
                    Arc::downgrade(&this_member),
                ));

                new_publish.add_receiver(Arc::downgrade(&new_self_play));

                publisher_participant.insert_publisher(
                    EndpointId(spec_play_endpoint.src.endpoint_id.to_string()),
                    new_publish,
                );

                self.insert_receiver(
                    EndpointId(spec_play_name.to_string()),
                    new_self_play,
                );
            }
        }

        // First of all, it is necessary to create [`WebRtcPublishEndpoint`]s
        // to which no [`WebRtcPlayEndpoint`] refers.
        this_member_spec.publish_endpoints().into_iter().for_each(
            |(name, e)| {
                let endpoint_id = EndpointId(name.clone());
                if self.publishers().get(&endpoint_id).is_none() {
                    self.insert_publisher(
                        endpoint_id,
                        Arc::new(WebRtcPublishEndpoint::new(
                            e.p2p.clone(),
                            Vec::new(),
                            Arc::downgrade(&this_member),
                        )),
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
        endpoint: Arc<WebRtcPlayEndpoint>,
    ) {
        self.0
            .lock()
            .unwrap()
            .borrow_mut()
            .receivers
            .insert(id, endpoint);
    }

    /// Insert publisher into this [`Participant`].
    pub fn insert_publisher(
        &self,
        id: EndpointId,
        endpoint: Arc<WebRtcPublishEndpoint>,
    ) {
        self.0
            .lock()
            .unwrap()
            .borrow_mut()
            .publishers
            .insert(id, endpoint);
    }

    /// Lookup [`WebRtcPublishEndpoint`] publisher by id.
    pub fn get_publisher_by_id(
        &self,
        id: &EndpointId,
    ) -> Option<Arc<WebRtcPublishEndpoint>> {
        self.0.lock().unwrap().borrow().publishers.get(id).cloned()
    }

    /// Lookup [`WebRtcPlayEndpoint`] receiver by id.
    pub fn get_receiver_by_id(
        &self,
        id: &EndpointId,
    ) -> Option<Arc<WebRtcPlayEndpoint>> {
        self.0.lock().unwrap().borrow().receivers.get(id).cloned()
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

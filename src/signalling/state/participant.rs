use std::{
    cell::RefCell,
    convert::TryFrom as _,
    sync::{Arc, Mutex},
};

use failure::Fail;
use hashbrown::HashMap;

use crate::api::control::{
    MemberId, MemberSpec, RoomSpec, TryFromElementError,
};

use super::endpoint::{
    Id as EndpointId, WebRtcPlayEndpoint, WebRtcPublishEndpoint,
};

#[derive(Debug, Fail)]
pub enum ParticipantsLoadError {
    #[fail(display = "TryFromElementError: {}", _0)]
    TryFromError(TryFromElementError),
    #[fail(display = "Member with id '{}' not found.", _0)]
    MemberNotFound(MemberId),
    #[fail(display = "Endpoint with id '{}' not found.", _0)]
    EndpointNotFound(String),
}

impl From<TryFromElementError> for ParticipantsLoadError {
    fn from(err: TryFromElementError) -> Self {
        ParticipantsLoadError::TryFromError(err)
    }
}

#[derive(Debug)]
pub struct Participant(Mutex<RefCell<ParticipantInner>>);

#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
pub struct ParticipantInner {
    id: MemberId,
    publishers: HashMap<EndpointId, Arc<WebRtcPublishEndpoint>>,
    receivers: HashMap<EndpointId, Arc<WebRtcPlayEndpoint>>,
    credentials: String,
}

impl Participant {
    fn new(id: MemberId, credentials: String) -> Self {
        Self(Mutex::new(RefCell::new(ParticipantInner {
            id,
            publishers: HashMap::new(),
            receivers: HashMap::new(),
            credentials,
        })))
    }

    pub fn id(&self) -> MemberId {
        self.0.lock().unwrap().borrow().id.clone()
    }

    pub fn load_store(
        room_spec: &RoomSpec,
    ) -> Result<HashMap<MemberId, Arc<Self>>, ParticipantsLoadError> {
        ParticipantInner::load_store(room_spec)
    }

    pub fn credentials(&self) -> String {
        self.0.lock().unwrap().borrow().credentials.clone()
    }

    pub fn publishers(
        &self,
    ) -> HashMap<EndpointId, Arc<WebRtcPublishEndpoint>> {
        self.0.lock().unwrap().borrow().publishers.clone()
    }

    pub fn receivers(&self) -> HashMap<EndpointId, Arc<WebRtcPlayEndpoint>> {
        self.0.lock().unwrap().borrow().receivers.clone()
    }

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

    pub fn get_publisher_by_id(
        &self,
        id: &EndpointId,
    ) -> Option<Arc<WebRtcPublishEndpoint>> {
        self.0.lock().unwrap().borrow().publishers.get(id).cloned()
    }
}

impl ParticipantInner {
    pub fn new(id: MemberId, credentials: String) -> Self {
        Self {
            id,
            publishers: HashMap::new(),
            receivers: HashMap::new(),
            credentials,
        }
    }

    pub fn load_store(
        room_spec: &RoomSpec,
    ) -> Result<HashMap<MemberId, Arc<Participant>>, ParticipantsLoadError>
    {
        let members = room_spec.members()?;
        let mut participants = HashMap::new();

        for (id, member) in &members {
            participants.insert(
                id.clone(),
                Arc::new(Participant::new(
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
}

// TODO (evdokimovs): add Participant unit tests

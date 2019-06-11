use std::convert::TryFrom;
use std::sync::{Arc, Mutex};

use crate::api::control::{
    MemberId, MemberSpec, RoomSpec, TryFromElementError,
};
use failure::Fail;
use hashbrown::HashMap;
use std::cell::RefCell;

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

#[derive(Debug)]
pub struct ParticipantInner {
    id: MemberId,
    send: HashMap<EndpointId, Arc<WebRtcPublishEndpoint>>,
    recv: HashMap<EndpointId, Arc<WebRtcPlayEndpoint>>,
    credentials: String,
}

impl Participant {
    pub fn new(id: MemberId, credentials: String) -> Self {
        Self(Mutex::new(RefCell::new(ParticipantInner {
            id,
            send: HashMap::new(),
            recv: HashMap::new(),
            credentials,
        })))
    }

    pub fn id(&self) -> MemberId {
        self.0.lock().unwrap().borrow().id.clone()
    }

    pub fn get_store(
        room_spec: &RoomSpec,
    ) -> Result<HashMap<MemberId, Arc<Self>>, ParticipantsLoadError> {
        ParticipantInner::get_store(room_spec)
    }

    pub fn credentials(&self) -> String {
        self.0.lock().unwrap().borrow().credentials.clone()
    }

    pub fn publish(&self) -> HashMap<EndpointId, Arc<WebRtcPublishEndpoint>> {
        self.0.lock().unwrap().borrow().send.clone()
    }

    pub fn receivers(&self) -> HashMap<EndpointId, Arc<WebRtcPlayEndpoint>> {
        self.0.lock().unwrap().borrow().recv.clone()
    }

    pub fn load(
        &self,
        room_spec: &RoomSpec,
        store: &HashMap<MemberId, Arc<Self>>,
    ) -> Result<(), ParticipantsLoadError> {
        let spec = MemberSpec::try_from(
            room_spec.pipeline.pipeline.get(&self.id().0).map_or(
                Err(ParticipantsLoadError::MemberNotFound(self.id())),
                Ok,
            )?,
        )?;

        let me = store.get(&self.id()).map_or(
            Err(ParticipantsLoadError::MemberNotFound(self.id())),
            Ok,
        )?;

        for (name_p, p) in spec.play_endpoints() {
            let sender_id = MemberId(p.src.member_id.to_string());
            let sender_participant = store.get(&sender_id).map_or(
                Err(ParticipantsLoadError::MemberNotFound(sender_id)),
                Ok,
            )?;
            let publisher_spec = MemberSpec::try_from(
                room_spec
                    .pipeline
                    .pipeline
                    .get(&p.src.member_id.to_string())
                    .map_or(
                        Err(ParticipantsLoadError::MemberNotFound(
                            p.src.member_id.clone(),
                        )),
                        Ok,
                    )?,
            )?;

            let publisher_endpoint = *publisher_spec
                .publish_endpoints()
                .get(&p.src.endpoint_id)
                .map_or(
                    Err(ParticipantsLoadError::EndpointNotFound(
                        p.src.endpoint_id.clone(),
                    )),
                    Ok,
                )?;

            if let Some(publisher) = sender_participant
                .get_publisher(&EndpointId(p.src.endpoint_id.to_string()))
            {
                let play_endpoint = Arc::new(WebRtcPlayEndpoint::new(
                    p.src.clone(),
                    Arc::downgrade(&publisher),
                    Arc::downgrade(&me),
                ));

                self.add_receiver(
                    EndpointId(name_p.to_string()),
                    Arc::clone(&play_endpoint),
                );

                publisher.add_receiver(Arc::downgrade(&play_endpoint));
            } else {
                let send_endpoint = Arc::new(WebRtcPublishEndpoint::new(
                    publisher_endpoint.p2p.clone(),
                    Vec::new(),
                    Arc::downgrade(&sender_participant),
                ));

                let play_endpoint = Arc::new(WebRtcPlayEndpoint::new(
                    p.src.clone(),
                    Arc::downgrade(&send_endpoint),
                    Arc::downgrade(&me),
                ));

                send_endpoint.add_receiver(Arc::downgrade(&play_endpoint));

                sender_participant.add_sender(
                    EndpointId(p.src.endpoint_id.to_string()),
                    send_endpoint,
                );

                self.add_receiver(
                    EndpointId(name_p.to_string()),
                    play_endpoint,
                );
            }
        }

        spec.publish_endpoints().into_iter().for_each(|(name, e)| {
            let endpoint_id = EndpointId(name.clone());
            if self.publish().get(&endpoint_id).is_none() {
                self.add_sender(
                    endpoint_id,
                    Arc::new(WebRtcPublishEndpoint::new(
                        e.p2p.clone(),
                        Vec::new(),
                        Arc::downgrade(&me),
                    )),
                );
            }
        });

        Ok(())
    }

    pub fn add_receiver(
        &self,
        id: EndpointId,
        endpoint: Arc<WebRtcPlayEndpoint>,
    ) {
        self.0
            .lock()
            .unwrap()
            .borrow_mut()
            .recv
            .insert(id, endpoint);
    }

    pub fn add_sender(
        &self,
        id: EndpointId,
        endpoint: Arc<WebRtcPublishEndpoint>,
    ) {
        self.0
            .lock()
            .unwrap()
            .borrow_mut()
            .send
            .insert(id, endpoint);
    }

    pub fn get_publisher(
        &self,
        id: &EndpointId,
    ) -> Option<Arc<WebRtcPublishEndpoint>> {
        self.0.lock().unwrap().borrow().send.get(id).cloned()
    }
}

impl ParticipantInner {
    pub fn new(id: MemberId, credentials: String) -> Self {
        Self {
            id,
            send: HashMap::new(),
            recv: HashMap::new(),
            credentials,
        }
    }

    pub fn get_store(
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

        //        println!("\n\n\n\n{:#?}\n\n\n\n", participants);

        Ok(participants)
    }
}

// TODO (evdokimovs): add Participant unit tests

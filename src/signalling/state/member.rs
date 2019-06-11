use std::convert::TryFrom;
use std::sync::{Arc, Mutex, Weak};

use crate::api::control::{endpoint::P2pMode, MemberId, MemberSpec, RoomSpec};
use hashbrown::HashMap;
use std::cell::RefCell;

use super::endpoint::{
    Id as EndpointId, WebRtcPlayEndpoint, WebRtcPublishEndpoint,
};

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

    pub fn get_store(room_spec: &RoomSpec) -> HashMap<MemberId, Arc<Self>> {
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
        store: &HashMap<MemberId, Arc<Participant>>,
    ) {
        let spec = MemberSpec::try_from(
            room_spec.pipeline.pipeline.get(&self.id().0).unwrap(),
        )
        .unwrap();

        let me = store.get(&self.id()).unwrap().clone();

        spec.play_endpoints().iter().for_each(|(name_p, p)| {
            let sender_participant =
                store.get(&MemberId(p.src.member_id.to_string())).unwrap();

            let publisher = WebRtcPublishEndpoint::new(
                P2pMode::Always,
                Vec::new(),
                Arc::downgrade(&sender_participant),
            );

            match sender_participant
                .get_publisher(&EndpointId(p.src.endpoint_id.to_string()))
            {
                Some(publisher) => {
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
                }
                None => {
                    let send_endpoint = Arc::new(WebRtcPublishEndpoint::new(
                        P2pMode::Always,
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
        });
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

    pub fn load(
        &mut self,
        room_spec: &RoomSpec,
        store: &HashMap<MemberId, Arc<Participant>>,
    ) {
        let spec = MemberSpec::try_from(
            room_spec.pipeline.pipeline.get(&self.id.0).unwrap(),
        )
        .unwrap();

        let me = store.get(&self.id).unwrap();

        spec.play_endpoints().iter().for_each(|(name_p, p)| {
            let sender_participant = store.get(&p.src.member_id).unwrap();
            let publisher_spec = MemberSpec::try_from(
                room_spec
                    .pipeline
                    .pipeline
                    .get(&p.src.member_id.to_string())
                    .unwrap(),
            )
            .unwrap();

            let publisher_endpoint = *publisher_spec
                .publish_endpoints()
                .get(&p.src.endpoint_id)
                .unwrap();

            let publisher = WebRtcPublishEndpoint::new(
                publisher_endpoint.p2p.clone(),
                Vec::new(),
                Arc::downgrade(&sender_participant),
            );

            match sender_participant
                .get_publisher(&EndpointId(p.src.endpoint_id.to_string()))
            {
                Some(publisher) => {
                    let play_endpoint = Arc::new(WebRtcPlayEndpoint::new(
                        p.src.clone(),
                        Arc::downgrade(&publisher),
                        Arc::downgrade(&me),
                    ));

                    me.add_receiver(
                        EndpointId(name_p.to_string()),
                        Arc::clone(&play_endpoint),
                    );

                    publisher.add_receiver(Arc::downgrade(&play_endpoint));
                }
                None => {
                    let send_endpoint = Arc::new(WebRtcPublishEndpoint::new(
                        P2pMode::Always,
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

                    me.add_receiver(
                        EndpointId(name_p.to_string()),
                        play_endpoint,
                    );
                }
            }
        });
    }

    pub fn get_store(
        room_spec: &RoomSpec,
    ) -> HashMap<MemberId, Arc<Participant>> {
        let members = room_spec.members().unwrap();
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
            participant.load(room_spec, &participants);
        }

        //        println!("\n\n\n\n{:#?}\n\n\n\n", participants);

        participants
    }
}

// TODO (evdokimovs): add Participant unit tests

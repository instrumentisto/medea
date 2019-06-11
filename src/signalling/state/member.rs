use std::convert::TryFrom;
use std::sync::{Arc, Mutex};

use crate::api::control::{MemberId, MemberSpec, RoomSpec};
use hashbrown::HashMap;
use std::cell::RefCell;

use super::endpoint::{
    Id as EndpointId, P2pMode, WebRtcPlayEndpoint, WebRtcPublishEndpoint,
};

#[derive(Debug, Clone)]
pub struct Participant(Arc<Mutex<RefCell<ParticipantInner>>>);

#[derive(Debug)]
pub struct ParticipantInner {
    id: MemberId,
    // TODO (evdokimovs): there is memory leak because Arc.
    send: HashMap<EndpointId, WebRtcPublishEndpoint>,
    // TODO (evdokimovs): there is memory leak because Arc.
    recv: HashMap<EndpointId, WebRtcPlayEndpoint>,
    credentials: String,
}

impl Participant {
    pub fn new(id: MemberId, credentials: String) -> Self {
        Self(Arc::new(Mutex::new(RefCell::new(ParticipantInner {
            id,
            send: HashMap::new(),
            recv: HashMap::new(),
            credentials,
        }))))
    }

    pub fn id(&self) -> MemberId {
        self.0.lock().unwrap().borrow().id.clone()
    }

    pub fn load(&self, room_spec: &RoomSpec, store: &HashMap<MemberId, Self>) {
        self.0.lock().unwrap().borrow_mut().load(room_spec, store);
    }

    pub fn get_store(room_spec: &RoomSpec) -> HashMap<MemberId, Self> {
        ParticipantInner::get_store(room_spec)
    }

    pub fn credentials(&self) -> String {
        self.0.lock().unwrap().borrow().credentials.clone()
    }

    pub fn publish(&self) -> HashMap<EndpointId, WebRtcPublishEndpoint> {
        self.0.lock().unwrap().borrow().send.clone()
    }

    pub fn receivers(&self) -> HashMap<EndpointId, WebRtcPlayEndpoint> {
        self.0.lock().unwrap().borrow().recv.clone()
    }

    pub fn add_receiver(&self, id: EndpointId, endpoint: WebRtcPlayEndpoint) {
        self.0
            .lock()
            .unwrap()
            .borrow_mut()
            .recv
            .insert(id, endpoint);
    }

    pub fn add_sender(&self, id: EndpointId, endpoint: WebRtcPublishEndpoint) {
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
    ) -> Option<WebRtcPublishEndpoint> {
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
        store: &HashMap<MemberId, Participant>,
    ) {
        let spec = MemberSpec::try_from(
            room_spec.pipeline.pipeline.get(&self.id.0).unwrap(),
        )
        .unwrap();

        let me = store.get(&self.id).unwrap();

        spec.play_endpoints().iter().for_each(|(name_p, p)| {
            let sender_participant =
                store.get(&MemberId(p.src.member_id.to_string())).unwrap();

            let publisher = WebRtcPublishEndpoint::new(
                P2pMode::Always,
                Vec::new(),
                sender_participant.clone(),
            );

            match sender_participant
                .get_publisher(&EndpointId(p.src.endpoint_id.to_string()))
            {
                Some(publisher) => {
                    let play_endpoint = WebRtcPlayEndpoint::new(
                        p.src.clone(),
                        publisher.clone(),
                        me.clone(),
                    );

                    me.add_receiver(
                        EndpointId(name_p.to_string()),
                        play_endpoint.clone(),
                    );

                    publisher.add_receiver(play_endpoint);
                }
                None => {
                    let send_endpoint = WebRtcPublishEndpoint::new(
                        P2pMode::Always,
                        Vec::new(),
                        sender_participant.clone(),
                    );

                    let play_endpoint = WebRtcPlayEndpoint::new(
                        p.src.clone(),
                        send_endpoint.clone(),
                        me.clone(),
                    );

                    send_endpoint.add_receiver(play_endpoint.clone());

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

    pub fn get_store(room_spec: &RoomSpec) -> HashMap<MemberId, Participant> {
        let members = room_spec.members().unwrap();
        let mut participants = HashMap::new();

        for (id, member) in &members {
            participants.insert(
                id.clone(),
                Participant::new(id.clone(), member.credentials().to_string())
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

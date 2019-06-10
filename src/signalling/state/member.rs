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
    send: HashMap<EndpointId, Arc<WebRtcPublishEndpoint>>,
    // TODO (evdokimovs): there is memory leak because Arc.
    recv: HashMap<EndpointId, Arc<WebRtcPlayEndpoint>>,
    credentials: String,
}

impl Participant {
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

    pub fn publish(&self) -> HashMap<String, Arc<WebRtcPublishEndpoint>> {
        self.0.lock().unwrap().borrow().send.clone()
    }

    pub fn receivers(&self) -> HashMap<String, Arc<WebRtcPlayEndpoint>> {
        self.0.lock().unwrap().borrow().recv.clone()
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

    pub fn add_recv_to_send(
        &self,
        id: EndpointId,
        endpoint: Arc<WebRtcPublishEndpoint>,
    ) {
        self.0
            .lock()
            .unwrap()
            .borrow_mut()
            .send
            .entry(id)
            .or_insert(Vec::new())
            .push(endpoint)
    }

    pub fn get_publisher(
        &self,
        id: &EndpointId,
    ) -> Option<Arc<WebRtcPublishEndpoint>> {
        self.0.lock().unwrap().borrow().send.get(id)
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
        spec.play_endpoints().iter().for_each(|(_, p)| {
            let sender_participant =
                store.get(&MemberId(p.src.member_id.to_string())).unwrap();

            let publisher = WebRtcPublishEndpoint {
                participant: sender_participant.clone(),
                receivers: Vec::new(),
                p2p: P2pMode::Always,
            };

            match sender_participant
                .get_publisher(&EndpointId(p.src.endpoint_id.to_string()))
            {
                Some(publisher) => {
                    WebRtcPlayEndpoint {
                        participant: me,
                        publisher,
                        src: p.src,
                    }

                    // TODO: Mutate publisher here
                }
                None => {
                    // Create Publisher
                    // create play
                    // add play to publisher
                    // insert publisher into participant
                }
            }

            self.recv.push(WebRtcPlayEndpoint {
                src: p.src,
                publisher: sender_participant.clone(),
            });
        });

        for (id, element) in room_spec.pipeline.iter() {
            let member = MemberSpec::try_from(element).unwrap();

            member
                .play_endpoints()
                .into_iter()
                .filter(|(_, endpoint)| endpoint.src.member_id == self_id)
                .for_each(|_| {
                    let member_id = MemberId(id.clone());
                    let participant = store.get(&member_id).unwrap().clone();
                });
        }
    }

    pub fn get_store(room_spec: &RoomSpec) -> HashMap<MemberId, Participant> {
        let members = room_spec.members().unwrap();
        let mut participants = HashMap::new();

        for (id, member) in &members {
            participants.insert(
                id.clone(),
                Participant(Arc::new(Mutex::new(RefCell::new(Self::new(
                    id.clone(),
                    member.credentials().to_string(),
                ))))),
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

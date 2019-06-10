use std::convert::TryFrom;
use std::sync::{Arc, Mutex};

use crate::api::control::{MemberId, MemberSpec, RoomSpec};
use hashbrown::HashMap;
use std::cell::RefCell;

pub struct Id(String);

#[derive(Debug, Clone)]
pub struct Participant(Arc<Mutex<RefCell<ParticipantInner>>>);

#[derive(Debug)]
pub struct ParticipantInner {
    id: MemberId,
    // TODO(evdokimovs): there is memory leak because Arc.
    senders: HashMap<MemberId, Participant>,
    // TODO(evdokimovs): there is memory leak because Arc.
    receivers: HashMap<MemberId, Participant>,
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

    pub fn publish(&self) -> HashMap<MemberId, Self> {
        self.0.lock().unwrap().borrow().senders.clone()
    }

    pub fn play(&self) -> HashMap<MemberId, Self> {
        self.0.lock().unwrap().borrow().receivers.clone()
    }
}

impl ParticipantInner {
    pub fn new(id: MemberId, credentials: String) -> Self {
        Self {
            id,
            senders: HashMap::new(),
            receivers: HashMap::new(),
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
        spec.play_endpoints().iter().for_each(|(_, p)| {
            let sender_participant =
                store.get(&MemberId(p.src.member_id.to_string())).unwrap();
            self.senders.insert(
                sender_participant.id().clone(),
                sender_participant.clone(),
            );
        });

        let self_id = self.id.clone();

        for (id, element) in room_spec.pipeline.iter() {
            let member = MemberSpec::try_from(element).unwrap();

            member
                .play_endpoints()
                .into_iter()
                .filter(|(_, endpoint)| endpoint.src.member_id == self_id)
                .for_each(|_| {
                    let member_id = MemberId(id.clone());
                    let participant = store.get(&member_id).unwrap().clone();
                    self.receivers.insert(member_id, participant);
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

        println!("\n\n\n\n{:#?}\n\n\n\n", participants);

        participants
    }
}

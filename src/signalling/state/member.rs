use std::convert::TryFrom;
use std::sync::{Arc, Mutex};

use crate::api::control::{
    MemberId, MemberSpec, RoomSpec,
};
use hashbrown::HashMap;
use std::cell::RefCell;

pub struct Id(String);

#[derive(Debug, Clone)]
pub struct Participant(Arc<Mutex<RefCell<ParticipantInner>>>);

#[derive(Debug)]
pub struct ParticipantInner {
    id: MemberId,
    senders: HashMap<MemberId, Participant>,
    receivers: HashMap<MemberId, Participant>,
    credentials: String,
}

impl Participant {
    pub fn id(&self) -> MemberId {
        self.0.lock().unwrap().borrow().id.clone()
    }

    pub fn load(
        &self,
        room_spec: &RoomSpec,
        store: &HashMap<MemberId, Self>,
    ) {
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
            self.receivers.insert(
                sender_participant.id().clone(),
                sender_participant.clone(),
            );
        });
    }

    pub fn get_store(room_spec: &RoomSpec) -> HashMap<MemberId, Participant> {
        let members = room_spec.members().unwrap();
        let mut participants = HashMap::new();

        for (id, member) in &members {
            participants.insert(
                id.clone(),
                Participant(Arc::new(Mutex::new(RefCell::new(
                    Self::new(
                        id.clone(),
                        member.credentials().to_string(),
                    ),
                )))),
            );
        }

        for (_, participant) in &participants {
            participant.load(room_spec, &participants);
        }

        participants
    }
}

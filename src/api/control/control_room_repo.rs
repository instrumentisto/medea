use super::ControlRoom;

use crate::signalling::RoomId;

use hashbrown::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct ControlRoomRepository {
    // TODO: Use crossbeam's concurrent hashmap when its done.
    //       [Tracking](https://github.com/crossbeam-rs/rfcs/issues/32).
    rooms: Arc<Mutex<HashMap<RoomId, ControlRoom>>>,
}

impl ControlRoomRepository {
    pub fn new(rooms: HashMap<RoomId, ControlRoom>) -> Self {
        Self {
            rooms: Arc::new(Mutex::new(rooms)),
        }
    }

    pub fn get(&self, id: RoomId) -> Option<ControlRoom> {
        let rooms = self.rooms.lock().unwrap();
        rooms.get(&id).cloned()
    }

    pub fn add(&mut self, room_id: RoomId, room: ControlRoom) {
        let mut rooms = self.rooms.lock().unwrap();
        rooms.insert(room_id, room);
    }
}

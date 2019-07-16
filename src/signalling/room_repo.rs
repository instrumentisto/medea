//! Repository that stores [`Room`]s addresses.

use std::sync::{Arc, Mutex};

use actix::Addr;
use hashbrown::HashMap;

use crate::{api::control::RoomId, signalling::Room};

/// Repository that stores [`Room`]s addresses.
#[derive(Clone, Debug)]
pub struct RoomRepository {
    // TODO: Use crossbeam's concurrent hashmap when its done.
    //       [Tracking](https://github.com/crossbeam-rs/rfcs/issues/32).
    rooms: Arc<Mutex<HashMap<RoomId, Addr<Room>>>>,
}

impl RoomRepository {
    /// Creates new [`Room`]s repository with passed-in [`Room`]s.
    pub fn new(rooms: HashMap<RoomId, Addr<Room>>) -> Self {
        Self {
            rooms: Arc::new(Mutex::new(rooms)),
        }
    }

    /// Returns [`Room`] by its ID.
    pub fn get(&self, id: &RoomId) -> Option<Addr<Room>> {
        let rooms = self.rooms.lock().unwrap();
        rooms.get(id).cloned()
    }

    pub fn remove(&self, id: &RoomId) {
        self.rooms.lock().unwrap().remove(id);
    }

    pub fn add(&self, id: RoomId, room: Addr<Room>) {
        self.rooms.lock().unwrap().insert(id, room);
    }

    pub fn is_contains_room_with_id(&self, id: &RoomId) -> bool {
        self.rooms.lock().unwrap().contains_key(id)
    }
}

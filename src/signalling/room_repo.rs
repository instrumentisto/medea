//! Repository that stores [`Room`]s addresses.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use actix::Addr;

use crate::signalling::{Room, RoomId};

/// Repository that stores [`Room`]s addresses.
#[derive(Clone, Default)]
pub struct RoomsRepository {
    // TODO: Use crossbeam's concurrent hashmap when its done.
    //       [Tracking](https://github.com/crossbeam-rs/rfcs/issues/32).
    rooms: Arc<Mutex<HashMap<RoomId, Addr<Room>>>>,
}

impl RoomsRepository {
    /// Creates new [`Room`]s repository with passed-in [`Room`]s.
    pub fn new(rooms: HashMap<RoomId, Addr<Room>>) -> Self {
        Self {
            rooms: Arc::new(Mutex::new(rooms)),
        }
    }

    /// Returns [`Room`] by its ID.
    pub fn get(&self, id: RoomId) -> Option<Addr<Room>> {
        let rooms = self.rooms.lock().unwrap();
        rooms.get(&id).cloned()
    }
}

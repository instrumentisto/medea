//! Repository that stores [`Room`]s addresses.

use std::{
    collections::HashMap as StdHashMap,
    sync::{Arc, Mutex},
};

use actix::{Actor, Addr, Arbiter, Context, Handler, Message};
use hashbrown::HashMap;

use crate::{
    api::control::model::{room::RoomSpec, RoomId},
    conf::Conf,
    signalling::Room,
    App,
};
use std::time::Duration;

/// Repository that stores [`Room`]s addresses.
#[derive(Clone, Debug)]
pub struct RoomsRepository {
    // TODO: Use crossbeam's concurrent hashmap when its done.
    //       [Tracking](https://github.com/crossbeam-rs/rfcs/issues/32).
    rooms: Arc<Mutex<HashMap<RoomId, Addr<Room>>>>,
    app: Arc<App>,
}

impl RoomsRepository {
    /// Creates new [`Room`]s repository with passed-in [`Room`]s.
    pub fn new(rooms: HashMap<RoomId, Addr<Room>>, app: Arc<App>) -> Self {
        Self {
            rooms: Arc::new(Mutex::new(rooms)),
            app,
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
}

impl Actor for RoomsRepository {
    type Context = Context<Self>;
}

// TODO: return sids.
#[derive(Message)]
#[rtype(result = "()")]
pub struct StartRoom<T: 'static + RoomSpec + Send> {
    pub room: T,
}

impl<T: 'static + RoomSpec + Send> Handler<StartRoom<T>> for RoomsRepository {
    type Result = ();

    fn handle(
        &mut self,
        msg: StartRoom<T>,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        let room_id = msg.room.id();

        let turn = Arc::clone(&self.app.turn_service);

        let room = Room::start_in_arbiter(&Arbiter::new(), move |_| {
            let room = msg.room;
            let room = Box::new(&room as &(RoomSpec));
            Room::new(&room, Duration::from_secs(10), turn).unwrap()
        });

        self.rooms.lock().unwrap().insert(room_id, room);
    }
}

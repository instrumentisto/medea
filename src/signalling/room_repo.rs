//! Repository that stores [`Room`]s addresses.

use std::sync::{Arc, Mutex};

use actix::{Actor, Addr, Context, Handler, Message};
use hashbrown::HashMap;

use crate::{
    api::control::{room::RoomSpec, RoomId},
    signalling::{
        room::{CloseRoom, RoomError},
        Room,
    },
    App,
};

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
#[rtype(result = "Result<(), RoomError>")]
pub struct StartRoom(pub RoomId, pub RoomSpec);

impl Handler<StartRoom> for RoomsRepository {
    type Result = Result<(), RoomError>;

    fn handle(
        &mut self,
        msg: StartRoom,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let room_id = msg.0;
        let room = msg.1;

        let turn = Arc::clone(&self.app.turn_service);

        let room = Room::new(
            &room,
            self.app.config.rpc.reconnect_timeout.clone(),
            turn,
        )?;
        let room_addr = room.start();

        self.rooms.lock().unwrap().insert(room_id, room_addr);
        Ok(())
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct DeleteRoom(pub RoomId);

impl Handler<DeleteRoom> for RoomsRepository {
    type Result = ();

    fn handle(
        &mut self,
        msg: DeleteRoom,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let mut is_need_remove = false;
        if let Some(room) = self.rooms.lock().unwrap().get(&msg.0) {
            room.do_send(CloseRoom {});
            is_need_remove = true;
        }
        if is_need_remove {
            self.remove(&msg.0);
        }
    }
}

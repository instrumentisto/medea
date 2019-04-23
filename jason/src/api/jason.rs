//! Main application handler. Responsible for managing shared transports,
//! local media, room initialization.
use wasm_bindgen::prelude::*;

use std::rc::Rc;

use crate::{
    api::{room::Room, RoomHandle},
    set_panic_hook,
    transport::Transport,
};

#[wasm_bindgen]
#[derive(Default)]
pub struct Jason {
    // TODO: multiple transports if rooms managed by different servers
    transport: Option<Rc<Transport>>,
    rooms: Vec<Room>,
}

#[wasm_bindgen]
impl Jason {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        set_panic_hook();
        Self::default()
    }

    /// Enter room with provided token, return initialized connection handler.
    /// TODO: Errors if unable to establish RPC connection with remote.
    pub fn join_room(&mut self, token: String) -> Result<RoomHandle, JsValue> {
        let mut transport = Transport::new(token, 3000);
        transport.init()?;
        let transport = Rc::new(transport);

        let room = Room::new(Rc::clone(&transport));
        room.subscribe(&transport);
        let handle = room.new_handle();

        self.rooms.push(room);
        self.transport = Some(transport);

        Ok(handle)
    }

    pub fn dispose(self) {}
}

//! Main application handler. Responsible for managing shared transports and
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
    sessions: Vec<Rc<Room>>,
}

#[wasm_bindgen]
impl Jason {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        set_panic_hook();
        Self::default()
    }

    /// Enter room with provided token, return initialized connection handler.
    /// Errors if unable to establish RPC connection with remote.
    pub fn join_room(&mut self, token: String) -> Result<RoomHandle, JsValue> {
        let mut transport = Transport::new(token, 3000);
        transport.init()?;
        let transport = Rc::new(transport);

        let session = Rc::new(Room::new(Rc::clone(&transport)));
        session.subscribe(&transport);

        self.sessions.push(Rc::clone(&session));
        self.transport = Some(transport);

        Ok(session.new_handle())
    }
}

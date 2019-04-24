//! Main application handler. Responsible for managing shared transports,
//! local media, room initialization.
use wasm_bindgen::prelude::*;

use std::rc::Rc;

use crate::{
    api::{room::Room, RoomHandle},
    rpc::RPCClient,
    set_panic_hook,
};

#[wasm_bindgen]
#[derive(Default)]
pub struct Jason {
    // TODO: multiple RPCClient's if rooms managed by different servers
    rpc: Option<Rc<RPCClient>>,
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
        let mut rpc = RPCClient::new(token, 3000);
        rpc.init()?;
        let rpc = Rc::new(rpc);

        let room = Room::new(Rc::clone(&rpc));
        room.subscribe(&rpc);
        let handle = room.new_handle();

        self.rooms.push(room);
        self.rpc = Some(rpc);

        Ok(handle)
    }

    pub fn dispose(self) {}
}

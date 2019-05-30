//! Main application handler. Responsible for managing shared transports,
//! local media, room initialization.
use futures::future::Future;
use js_sys::Promise;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

use std::{cell::RefCell, rc::Rc};

use crate::{
    api::room::Room,
    media::{MediaManager, MediaStreamHandle},
    rpc::RPCClient,
    set_panic_hook,
    utils::{Callback, WasmErr},
};

#[wasm_bindgen]
#[derive(Default)]
pub struct Jason(Rc<RefCell<Inner>>);

#[derive(Default)]
pub struct Inner {
    // TODO: multiple RPCClient's if rooms managed by different servers
    rpc: Option<Rc<RPCClient>>,
    media_manager: Rc<MediaManager>,
    rooms: Vec<Room>,
}

/// Main application handler. Responsible for managing shared transports,
/// local media, room initialization.
#[wasm_bindgen]
impl Jason {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        set_panic_hook();
        Self::default()
    }

    /// Enter room with provided token. Will establish connection with Medea
    /// server (if it doesn't already exist). Fails if unable to connect to
    /// Medea. Effectively returns Result<RoomHandle, WasmErr>
    pub fn join_room(&self, token: String) -> Promise {
        let mut rpc = RPCClient::new(token, 3000);
        let media_manager = Rc::clone(&self.0.borrow().media_manager);

        let inner = Rc::clone(&self.0);
        let fut = rpc
            .init()
            .and_then(move |()| {
                let rpc = Rc::new(rpc);
                let room = Room::new(&rpc, &media_manager);

                let handle = room.new_handle();

                inner.borrow_mut().rpc.replace(rpc);
                inner.borrow_mut().rooms.push(room);

                Ok(JsValue::from(handle))
            })
            .map_err(JsValue::from);

        future_to_promise(fut)
    }

    pub fn on_local_stream(&self, f: js_sys::Function) {
        self.0.borrow_mut().media_manager.on_local_stream(f);
    }

    /// Drops Jason and all related objects (Rooms, Connections, Streams etc. ).
    /// All objects related to this Jason instance will be detached (you will
    /// still hold them, but unable to use).
    pub fn dispose(self) {}
}

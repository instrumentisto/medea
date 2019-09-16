//! External Jason API accessible from JS.

mod connection;
mod room;

use std::{cell::RefCell, rc::Rc};

use futures::future::Future;
use js_sys::Promise;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

use crate::{
    media::MediaManager,
    peer,
    rpc::{RpcClient, WebsocketRpcClient},
    set_panic_hook,
};

#[doc(inline)]
pub use self::{connection::ConnectionHandle, room::Room, room::RoomHandle};

#[wasm_bindgen]
#[derive(Default)]
pub struct Jason(Rc<RefCell<Inner>>);

#[derive(Default)]
struct Inner {
    media_manager: Rc<MediaManager>,
    rooms: Vec<Room>,
}

/// Main library handler.
///
/// Responsible for managing shared transports, local media
/// and room initialization.
#[wasm_bindgen]
impl Jason {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        set_panic_hook();
        Self::default()
    }

    /// Returns [`RoomHandle`] for [`Room`] with preconfigured the authorization
    /// `token` for connection with media server.
    pub fn init_room(&self) -> JsValue {
        let rpc = Rc::new(WebsocketRpcClient::new(3000));
        let peer_repository = Box::new(peer::Repository::new(Rc::clone(
            &self.0.borrow().media_manager,
        )));

        let room = Room::new(rpc, peer_repository);

        let handle = room.new_handle();

        self.0.borrow_mut().rooms.push(room);

        JsValue::from(handle)
    }

    /// Sets `on_local_stream` callback, which will be invoked once media
    /// acquisition request will resolve returning
    /// [`MediaStreamHandle`](crate::media::MediaStreamHandle) or
    /// [`WasmErr`](crate::utils::errors::WasmErr).
    pub fn on_local_stream(&self, f: js_sys::Function) {
        self.0.borrow_mut().media_manager.set_on_local_stream(f);
    }

    pub fn media_manager(&self) -> JsValue {
        self.0.borrow().media_manager.new_handle().into()
    }

    /// Drops [`Jason`] API object, so all related objects (rooms, connections,
    /// streams etc.) respectively. All objects related to this [`Jason`] API
    /// object will be detached (you will still hold them, but unable to use).
    pub fn dispose(self) {}
}

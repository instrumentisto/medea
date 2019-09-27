//! External Jason API accessible from JS.

mod connection;
mod room;

use std::{cell::RefCell, rc::Rc};

use wasm_bindgen::prelude::*;

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
    // TODO: multiple RpcClient's if rooms managed by different servers
    rpc: Option<Rc<dyn RpcClient>>,
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
        Jason::default()
    }

    /// Performs entering to a [`Room`] by the authorization `token`.
    ///
    /// Establishes connection with media server (if it doesn't already exist).
    /// Fails if unable to connect to media server.
    pub async fn join_room(&self, token: String) -> Result<(), JsValue> {
//        let mut rpc = WebsocketRpcClient::new(token, 3000);
//        let peer_repository =
//            peer::Repository::new(Rc::clone(&self.0.borrow().media_manager));

//        rpc.init().await?;

//        let rpc: Rc<dyn RpcClient> = Rc::new(rpc);
//        let room = Room::new(Rc::clone(&rpc), Box::new(peer_repository));
//
//        let handle = room.new_handle();

//        self.0.borrow_mut().rpc.replace(rpc);
//        self.0.borrow_mut().rooms.push(room);

//        Ok(room.new_handle())
        Ok(())
    }

//    /// Sets `on_local_stream` callback, which will be invoked once media
//    /// acquisition request will resolve returning
//    /// [`MediaStreamHandle`](crate::media::MediaStreamHandle) or
//    /// [`WasmErr`](crate::utils::errors::WasmErr).
//    pub fn on_local_stream(&self, f: js_sys::Function) {
//        self.0.borrow_mut().media_manager.set_on_local_stream(f);
//    }
//
//    /// Drops [`Jason`] API object, so all related objects (rooms, connections,
//    /// streams etc.) respectively. All objects related to this [`Jason`] API
//    /// object will be detached (you will still hold them, but unable to use).
//    pub fn dispose(self) {}
}

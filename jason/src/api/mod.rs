//! External Jason API accessible from JS.

mod connection;
mod room;
mod room_stream;

use std::{cell::RefCell, rc::Rc};

use wasm_bindgen::prelude::*;

use crate::{
    media::{MediaManager, MediaManagerHandle},
    peer,
    rpc::WebsocketRpcClient,
    set_panic_hook,
};

#[doc(inline)]
pub use self::{
    connection::ConnectionHandle,
    room::{Room, RoomHandle},
    room_stream::RoomStream,
};

/// General library interface.
///
/// Responsible for managing shared transports, local media
/// and room initialization.
#[wasm_bindgen]
#[derive(Default)]
pub struct Jason(Rc<RefCell<Inner>>);

#[derive(Default)]
struct Inner {
    media_manager: Rc<MediaManager>,
    rooms: Vec<Room>,
}

#[wasm_bindgen]
impl Jason {
    /// Instantiates new [`Jason`] interface to interact with this library.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        set_panic_hook();
        Self::default()
    }

    /// Returns [`RoomHandle`] for [`Room`].
    pub fn init_room(&self) -> RoomHandle {
        let rpc = Rc::new(WebsocketRpcClient::new(3000));
        let peer_repository = Box::new(peer::Repository::default());
        let stream_source =
            Rc::new(RoomStream::new(Rc::clone(&self.0.borrow().media_manager)));
        let room = Room::new(rpc, peer_repository, stream_source);
        let handle = room.new_handle();
        self.0.borrow_mut().rooms.push(room);
        handle
    }

    /// Returns handle to [`MediaManager`].
    pub fn media_manager(&self) -> MediaManagerHandle {
        self.0.borrow().media_manager.new_handle()
    }

    /// Drops [`Jason`] API object, so all related objects (rooms, connections,
    /// streams etc.) respectively. All objects related to this [`Jason`] API
    /// object will be detached (you will still hold them, but unable to use).
    pub fn dispose(self) {}
}

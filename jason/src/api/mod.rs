//! External Jason API accessible from JS.

mod connection;
mod room;

use std::{cell::RefCell, rc::Rc};

use futures::FutureExt as _;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::{
    media::{MediaManager, MediaManagerHandle},
    peer,
    rpc::{
        ClientDisconnect, RpcClient as _, RpcTransport, WebSocketRpcClient,
        WebSocketRpcTransport,
    },
    set_panic_hook,
};

#[doc(inline)]
pub use self::{
    connection::{ConnectionHandle, Connections},
    room::Room,
    room::RoomHandle,
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
        wasm_logger::init(wasm_logger::Config::default());
        Self::default()
    }

    /// Returns [`RoomHandle`] for [`Room`].
    pub fn init_room(&self) -> RoomHandle {
        let rpc = Rc::new(WebSocketRpcClient::new(Box::new(|token| {
            Box::pin(async move {
                let ws = WebSocketRpcTransport::new(&token)
                    .await
                    .map_err(|e| tracerr::new!(e))?;
                Ok(Rc::new(ws) as Rc<dyn RpcTransport>)
            })
        })));
        let peer_repository = Box::new(peer::Repository::new(Rc::clone(
            &self.0.borrow().media_manager,
        )));

        spawn_local({
            let rpc = Rc::clone(&rpc);
            let inner = self.0.clone();
            async move {
                let reason = rpc.on_normal_close().await.unwrap_or_else(|_| {
                    ClientDisconnect::RpcClientUnexpectedlyDropped.into()
                });
                // TODO: Don't close all rooms when multiple rpc connections
                //       will be supported.
                inner
                    .borrow_mut()
                    .rooms
                    .drain(..)
                    .for_each(|room| room.close(reason));
                inner.borrow_mut().media_manager = Rc::default();
            }
        });

        let room = Room::new(rpc, peer_repository);
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
    pub fn dispose(self) {
        self.0.borrow_mut().rooms.drain(..).for_each(|room| {
            room.close(ClientDisconnect::RoomClosed.into());
        });
    }
}

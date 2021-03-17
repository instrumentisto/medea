//! External Jason API accessible from JS.

mod connection;
mod room;

use std::{cell::RefCell, rc::Rc};

use futures::FutureExt as _;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::{
    media::{MediaManager, MediaManagerHandle},
    rpc::{
        ClientDisconnect, RpcSession, RpcTransport, WebSocketRpcClient,
        WebSocketRpcSession, WebSocketRpcTransport,
    },
    set_panic_hook,
};

#[doc(inline)]
pub use self::{
    connection::{Connection, ConnectionHandle, Connections},
    room::{
        ConstraintsUpdateException, Room, RoomCloseReason, RoomError,
        RoomHandle, WeakRoom,
    },
};

/// General library interface.
///
/// Responsible for managing shared transports, local media
/// and room initialization.
#[wasm_bindgen]
pub struct Jason(Rc<RefCell<Inner>>);

struct Inner {
    /// [`Jason`]s [`MediaManager`]. It's shared across [`Room`]s since
    /// [`MediaManager`] contains media tracks that can be used by multiple
    /// [`Room`]s.
    media_manager: Rc<MediaManager>,

    /// [`Room`]s maintained by this [`Jason`] instance.
    rooms: Vec<Room>,

    /// Connection with Media Server. Only one [`WebSocketRpcClient`] is
    /// supported at the moment.
    rpc: Rc<WebSocketRpcClient>,
}

#[wasm_bindgen]
impl Jason {
    /// Instantiates new [`Jason`] interface to interact with this library.
    #[must_use]
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        set_panic_hook();
        wasm_logger::init(wasm_logger::Config::default());

        Self::with_rpc_client(Rc::new(WebSocketRpcClient::new(Box::new(
            |url| {
                Box::pin(async move {
                    let ws = WebSocketRpcTransport::new(url)
                        .await
                        .map_err(|e| tracerr::new!(e))?;
                    Ok(Rc::new(ws) as Rc<dyn RpcTransport>)
                })
            },
        ))))
    }

    /// Creates new [`Room`] and returns its [`RoomHandle`].
    #[must_use]
    pub fn init_room(&self) -> RoomHandle {
        let rpc = Rc::clone(&self.0.borrow().rpc);
        self.inner_init_room(WebSocketRpcSession::new(rpc))
    }

    /// Returns [`MediaManagerHandle`].
    #[must_use]
    pub fn media_manager(&self) -> MediaManagerHandle {
        self.0.borrow().media_manager.new_handle()
    }

    /// Closes the provided [`RoomHandle`].
    #[allow(clippy::needless_pass_by_value)]
    pub fn close_room(&self, room_to_delete: RoomHandle) {
        self.0.borrow_mut().rooms.retain(|room| {
            let should_be_closed = room.inner_ptr_eq(&room_to_delete);
            if should_be_closed {
                room.set_close_reason(ClientDisconnect::RoomClosed.into());
            }

            !should_be_closed
        });
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

impl Jason {
    /// Returns new [`Jason`] with the provided [`WebSocketRpcClient`].
    #[inline]
    pub fn with_rpc_client(rpc: Rc<WebSocketRpcClient>) -> Self {
        Self(Rc::new(RefCell::new(Inner {
            rpc,
            rooms: Vec::new(),
            media_manager: Rc::new(MediaManager::default()),
        })))
    }

    /// Returns [`RoomHandle`] for [`Room`].
    pub fn inner_init_room(&self, rpc: Rc<dyn RpcSession>) -> RoomHandle {
        let on_normal_close = rpc.on_normal_close();
        let room = Room::new(rpc, Rc::clone(&self.0.borrow().media_manager));

        let weak_room = room.downgrade();
        let weak_inner = Rc::downgrade(&self.0);
        spawn_local(on_normal_close.map(move |reason| {
            (|| {
                let room = weak_room.upgrade()?;
                let inner = weak_inner.upgrade()?;
                let mut inner = inner.borrow_mut();
                let index = inner.rooms.iter().position(|r| r.ptr_eq(&room));
                if let Some(index) = index {
                    inner.rooms.remove(index).close(reason);
                }
                if inner.rooms.is_empty() {
                    inner.media_manager = Rc::default();
                }

                Some(())
            })();
        }));

        let handle = room.new_handle();
        self.0.borrow_mut().rooms.push(room);
        handle
    }
}

impl Default for Jason {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

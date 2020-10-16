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
        ClientDisconnect, RpcSession, RpcTransport, Session,
        WebSocketRpcClient, WebSocketRpcTransport,
    },
    set_panic_hook,
};

#[doc(inline)]
pub use self::{
    connection::{ConnectionHandle, Connections},
    room::Room,
    room::RoomHandle,
};

struct Inner {
    /// Connection with `Media Server`. Only one [`WebSocketRpcClient`] is
    /// supported atm.
    rpc: Rc<WebSocketRpcClient>,

    /// [`Room`]s maintained by this [`Jason`] instance.
    rooms: Vec<Room>,

    /// [`Jason`]s [`MediaManager`]. It is shared across [`Room`]s since
    /// [`MediaManager`] contains media tracks that can be used by multiple
    /// [`Room`]s.
    media_manager: Rc<MediaManager>,
}

/// General library interface.
///
/// Responsible for managing shared transports, local media
/// and room initialization.
#[wasm_bindgen]
pub struct Jason(Rc<RefCell<Inner>>);

#[wasm_bindgen]
impl Jason {
    /// Instantiates new [`Jason`] interface to interact with this library.
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

    /// Returns [`RoomHandle`] for [`Room`].
    pub fn init_room(&self) -> RoomHandle {
        let rpc = Rc::clone(&self.0.borrow().rpc);
        self.inner_init_room(Session::new(rpc))
    }

    /// Returns handle to [`MediaManager`].
    pub fn media_manager(&self) -> MediaManagerHandle {
        self.0.borrow().media_manager.new_handle()
    }

    /// Drops [`Room`] with a provided ID.
    ///
    /// Sets [`Room`]'s close reason to [`ClientDisconnect::RoomClose`].
    pub fn dispose_room(&self, room: RoomHandle) {
        self.0.borrow_mut().rooms.retain(|room| {
            let should_be_closed =
                room.id().map_or(false, |id| id.0 == room_id);
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
    /// Returns new [`Jason`] with a provided [`WebSocketRpcClient`].
    pub fn with_rpc_client(rpc: Rc<WebSocketRpcClient>) -> Self {
        Self(Rc::new(RefCell::new(Inner {
            rpc,
            rooms: Vec::new(),
            media_manager: Rc::new(MediaManager::default()),
        })))
    }

    /// Returns [`RoomHandle`] for [`Room`].
    pub fn inner_init_room(&self, rpc: Rc<dyn RpcSession>) -> RoomHandle {
        let peer_repository = Box::new(peer::Repository::new(Rc::clone(
            &self.0.borrow().media_manager,
        )));
        let on_normal_close = rpc.on_normal_close();
        let room = Room::new(rpc, peer_repository);

        let weak_room = room.downgrade();
        let weak_inner = Rc::downgrade(&self.0);
        spawn_local(on_normal_close.map(move |res| {
            (|| {
                let room = weak_room.upgrade()?;
                let inner = weak_inner.upgrade()?;
                let reason = res.unwrap_or_else(|_| {
                    ClientDisconnect::RpcClientUnexpectedlyDropped.into()
                });
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
    fn default() -> Self {
        Self::new()
    }
}

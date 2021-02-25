//! External Jason API accessible from JS.

use futures::FutureExt as _;
use std::{cell::RefCell, rc::Rc};

use crate::platform;

use self::{
    media::MediaManager,
    rpc::{
        ClientDisconnect, RpcSession, WebSocketRpcClient, WebSocketRpcSession,
    },
};

#[doc(inline)]
pub use self::{
    connection::{ConnectionHandle, Connections},
    media::{
        track::{local::LocalMediaTrack, remote, MediaStreamTrackState},
        AudioTrackConstraints, DeviceVideoTrackConstraints,
        DisplayVideoTrackConstraints, MediaKind, MediaManagerHandle,
        MediaSourceKind, MediaStreamSettings,
    },
    room::{
        ConstraintsUpdateException, Room, RoomCloseReason, RoomError,
        RoomHandle,
    },
    rpc::ReconnectHandle,
};

#[macro_use]
pub mod utils;
mod connection;
pub mod media;
pub mod peer;
mod room;
pub mod rpc;

/// General library interface.
///
/// Responsible for managing shared transports, local media
/// and room initialization.
pub struct Jason(Rc<RefCell<Inner>>);

pub struct Inner {
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

impl Jason {
    /// Instantiates new [`Jason`] interface to interact with this library.
    pub fn new() -> Self {
        Self::with_rpc_client(Rc::new(WebSocketRpcClient::new(Box::new(
            |url| {
                Box::pin(async move {
                    let ws = platform::WebSocketRpcTransport::new(url)
                        .await
                        .map_err(|e| tracerr::new!(e))?;
                    Ok(Rc::new(ws) as Rc<dyn platform::RpcTransport>)
                })
            },
        ))))
    }

    /// Creates new [`Room`] and returns its [`RoomHandle`].
    pub fn init_room(&self) -> RoomHandle {
        let rpc = Rc::clone(&self.0.borrow().rpc);
        self.inner_init_room(WebSocketRpcSession::new(rpc))
    }

    /// Returns [`MediaManagerHandle`].
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
        platform::spawn(on_normal_close.map(move |reason| {
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

//! Connection with specific remote `Member`.

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::{Rc, Weak},
};

use medea_client_api_proto::{MemberId, PeerId, TrackId};
use wasm_bindgen::prelude::*;

use crate::{
    media::MediaStreamTrack,
    peer::{PeerMediaStream, RemoteMediaStream},
    utils::{Callback0, Callback1, HandlerDetachedError},
};

/// Service which manages [`Connection`]s with the remote `Member`s.
#[derive(Default)]
pub struct Connections {
    /// Local [`PeerId`] to remote [`MemberId`].
    peer_members: RefCell<HashMap<PeerId, HashSet<MemberId>>>,

    /// Remote [`MemberId`] to [`Connection`] with that `Member`.
    connections: RefCell<HashMap<MemberId, Connection>>,

    /// Callback from JS side which will be invoked on remote `Member` media
    /// stream arrival.
    on_new_connection: Callback1<ConnectionHandle>,
}

impl Connections {
    /// Sets callback, which will be invoked when new [`Connection`] is
    /// established.
    pub fn on_new_connection(&self, f: js_sys::Function) {
        self.on_new_connection.set_func(f);
    }

    /// Creates new connection with remote `Member` based on its [`MemberId`].
    ///
    /// No-op if [`Connection`] already exists.
    pub fn create_connection(
        &self,
        local_peer_id: PeerId,
        remote_member_id: &MemberId,
    ) {
        let is_new = !self.connections.borrow().contains_key(remote_member_id);
        if is_new {
            let con = Connection::new(remote_member_id.clone());
            self.on_new_connection.call(con.new_handle());
            self.connections
                .borrow_mut()
                .insert(remote_member_id.clone(), con);
            self.peer_members
                .borrow_mut()
                .entry(local_peer_id)
                .or_default()
                .insert(remote_member_id.clone());
        }
    }

    /// Lookups [`Connection`] by the given remote [`PeerId`].
    pub fn get(&self, remote_member_id: &MemberId) -> Option<Connection> {
        self.connections.borrow().get(remote_member_id).cloned()
    }

    /// Closes [`Connection`] associated with provided local [`PeerId`].
    ///
    /// Invokes `on_close` callback.
    pub fn close_connection(&self, local_peer: PeerId) {
        if let Some(remote_ids) =
            self.peer_members.borrow_mut().remove(&local_peer)
        {
            for remote_id in remote_ids {
                if let Some(connection) =
                    self.connections.borrow_mut().remove(&remote_id)
                {
                    // `on_close` callback is invoked here and not in `Drop`
                    // implementation so `ConnectionHandle` is available during
                    // callback invocation.
                    connection.0.on_close.call();
                }
            }
        }
    }
}

/// Connection with a specific remote `Member`, that is used on JS side.
///
/// Actually, represents a [`Weak`]-based handle to `InnerConnection`.
#[wasm_bindgen]
pub struct ConnectionHandle(Weak<InnerConnection>);

/// Actual data of a connection with a specific remote [`Member`].
///
/// Shared between JS side ([`ConnectionHandle`]) and Rust side
/// ([`Connection`]).
struct InnerConnection {
    /// Remote [`Member`] ID.
    remote_id: MemberId,

    /// [`PeerMediaStream`] received from remote member.
    remote_stream: RefCell<Option<PeerMediaStream>>,

    /// JS callback, that will be invoked when remote [`PeerMediaStream`] is
    /// received.
    on_remote_stream: Callback1<RemoteMediaStream>,

    /// JS callback, that will be invoked when this connection is closed.
    on_close: Callback0,
}

#[wasm_bindgen]
impl ConnectionHandle {
    /// Sets callback, which will be invoked as soon as first media track from
    /// remote `Member` is received.
    ///
    /// It's guaranteed that provided stream will have at least one media track
    /// when this callback is fired. List of tracks in provided stream is not
    /// final and can be changed in future.
    pub fn on_remote_stream(&self, f: js_sys::Function) -> Result<(), JsValue> {
        upgrade_or_detached!(self.0)
            .map(|inner| inner.on_remote_stream.set_func(f))
    }

    /// Sets callback, which will be invoked when this `Connection` will close.
    pub fn on_close(&self, f: js_sys::Function) -> Result<(), JsValue> {
        upgrade_or_detached!(self.0).map(|inner| inner.on_close.set_func(f))
    }

    /// Returns remote `Member` ID.
    pub fn get_remote_member_id(&self) -> Result<String, JsValue> {
        upgrade_or_detached!(self.0).map(|inner| inner.remote_id.0.clone())
    }
}

/// Connection with a specific remote [`Member`], that is used on Rust side.
///
/// Actually, represents a handle to [`InnerConnection`].
#[derive(Clone)]
pub struct Connection(Rc<InnerConnection>);

impl Connection {
    /// Instantiates new [`Connection`] for a given [`Member`].
    #[inline]
    pub fn new(remote_id: MemberId) -> Self {
        Self(Rc::new(InnerConnection {
            remote_id,
            remote_stream: RefCell::new(None),
            on_remote_stream: Callback1::default(),
            on_close: Callback0::default(),
        }))
    }

    /// Adds provided [`MediaStreamTrack`] to remote stream of this
    /// [`Connection`].
    ///
    /// If this is the first track added to this [`Connection`], then a new
    /// [`PeerMediaStream`] is built and sent to `on_remote_stream` callback.
    pub fn add_remote_track(&self, track_id: TrackId, track: MediaStreamTrack) {
        let is_new_stream = self.0.remote_stream.borrow().is_none();
        let mut remote_stream_ref = self.0.remote_stream.borrow_mut();
        let stream = remote_stream_ref.get_or_insert_with(PeerMediaStream::new);
        stream.add_track(track_id, track);

        if is_new_stream {
            self.0.on_remote_stream.call(stream.new_handle());
        }
    }

    /// Creates new [`ConnectionHandle`] for using [`Connection`] on JS side.
    #[inline]
    pub fn new_handle(&self) -> ConnectionHandle {
        ConnectionHandle(Rc::downgrade(&self.0))
    }
}

//! [`Connection`] with a specific remote `Member`.

use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::{Rc, Weak},
};

use derive_more::Display;
use medea_client_api_proto::{ConnectionQualityScore, MemberId, PeerId};
use tracerr::Traced;

use crate::{api, media::track::remote, platform, utils::JsCaused};

/// Service which manages [`Connection`]s with remote `Member`s.
#[derive(Default)]
pub struct Connections {
    /// Local [`PeerId`] to remote [`MemberId`].
    peer_members: RefCell<HashMap<PeerId, HashSet<MemberId>>>,

    /// Remote [`MemberId`] to [`Connection`] with that `Member`.
    connections: RefCell<HashMap<MemberId, Connection>>,

    /// Callback invoked on remote `Member` media arrival.
    on_new_connection: platform::Callback<api::ConnectionHandle>,
}

impl Connections {
    /// Sets callback, which will be invoked when new [`Connection`] is
    /// established.
    pub fn on_new_connection(
        &self,
        f: platform::Function<api::ConnectionHandle>,
    ) {
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
            self.on_new_connection.call1(con.new_handle());
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
                    connection.0.on_close.call0();
                }
            }
        }
    }
}

/// Error of [`ConnectionHandle`]'s [`Weak`] pointer being detached.
#[derive(Clone, Copy, Debug, Display, JsCaused)]
#[js(error = "platform::Error")]
#[display(fmt = "ConnectionHandle is in detached state")]
pub struct HandlerDetachedError;

/// External handler to a [`Connection`] with a remote `Member`.
///
/// Actually, represents a [`Weak`]-based handle to `InnerConnection`.
pub struct ConnectionHandle(Weak<InnerConnection>);

/// Actual data of a connection with a specific remote `Member`.
///
/// Shared between external [`ConnectionHandle`] and Rust side [`Connection`].
struct InnerConnection {
    /// Remote `Member` ID.
    remote_id: MemberId,

    /// Current [`ConnectionQualityScore`] of this [`Connection`].
    quality_score: Cell<Option<ConnectionQualityScore>>,

    /// Callback invoked when a [`remote::Track`] is received.
    on_remote_track_added: platform::Callback<api::RemoteMediaTrack>,

    /// Callback invoked when a [`ConnectionQualityScore`] is updated.
    on_quality_score_update: platform::Callback<u8>,

    /// Callback invoked when this [`Connection`] is closed.
    on_close: platform::Callback<()>,
}

impl ConnectionHandle {
    /// Sets callback, invoked when this `Connection` will close.
    ///
    /// # Errors
    ///
    /// See [`HandlerDetachedError`] for details.
    pub fn on_close(
        &self,
        f: platform::Function<()>,
    ) -> Result<(), Traced<HandlerDetachedError>> {
        self.0
            .upgrade()
            .ok_or_else(|| tracerr::new!(HandlerDetachedError))
            .map(|inner| inner.on_close.set_func(f))
    }

    /// Returns remote `Member` ID.
    ///
    /// # Errors
    ///
    /// See [`HandlerDetachedError`] for details.
    pub fn get_remote_member_id(
        &self,
    ) -> Result<String, Traced<HandlerDetachedError>> {
        self.0
            .upgrade()
            .ok_or_else(|| tracerr::new!(HandlerDetachedError))
            .map(|inner| inner.remote_id.0.clone())
    }

    /// Sets callback, invoked when a new [`remote::Track`] is added to this
    /// [`Connection`].
    ///
    /// # Errors
    ///
    /// See [`HandlerDetachedError`] for details.
    pub fn on_remote_track_added(
        &self,
        f: platform::Function<api::RemoteMediaTrack>,
    ) -> Result<(), Traced<HandlerDetachedError>> {
        self.0
            .upgrade()
            .ok_or_else(|| tracerr::new!(HandlerDetachedError))
            .map(|inner| inner.on_remote_track_added.set_func(f))
    }

    /// Sets callback, invoked when a connection quality score is updated by
    /// a server.
    ///
    /// # Errors
    ///
    /// See [`HandlerDetachedError`] for details.
    pub fn on_quality_score_update(
        &self,
        f: platform::Function<u8>,
    ) -> Result<(), Traced<HandlerDetachedError>> {
        self.0
            .upgrade()
            .ok_or_else(|| tracerr::new!(HandlerDetachedError))
            .map(|inner| inner.on_quality_score_update.set_func(f))
    }
}

/// Connection with a specific remote `Member`, that is used on Rust side.
#[derive(Clone)]
pub struct Connection(Rc<InnerConnection>);

impl Connection {
    /// Instantiates new [`Connection`] for a given `Member`.
    #[inline]
    #[must_use]
    pub fn new(remote_id: MemberId) -> Self {
        Self(Rc::new(InnerConnection {
            remote_id,
            quality_score: Cell::default(),
            on_quality_score_update: platform::Callback::default(),
            on_close: platform::Callback::default(),
            on_remote_track_added: platform::Callback::default(),
        }))
    }

    /// Invokes `on_remote_track_added` callback with the provided
    /// [`remote::Track`].
    pub fn add_remote_track(&self, track: remote::Track) {
        self.0.on_remote_track_added.call1(track);
    }

    /// Creates a new external handle to this [`Connection`].
    #[inline]
    #[must_use]
    pub fn new_handle(&self) -> ConnectionHandle {
        ConnectionHandle(Rc::downgrade(&self.0))
    }

    /// Updates [`ConnectionQualityScore`] of this [`Connection`].
    pub fn update_quality_score(&self, score: ConnectionQualityScore) {
        if self.0.quality_score.replace(Some(score)) != Some(score) {
            self.0.on_quality_score_update.call1(score as u8);
        }
    }
}

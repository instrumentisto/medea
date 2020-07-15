//! Connection with specific remote `Member`.

use std::{
    cell::RefCell,
    collections::HashMap,
    rc::{Rc, Weak},
};

use futures::{stream::LocalBoxStream, StreamExt};
use medea_client_api_proto::{Direction, PeerId, Track, TrackId};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::{
    media::MediaStreamTrack,
    peer::{
        MuteState, MuteStateUpdate, MuteStateUpdatesPublisher, PeerMediaStream,
        RemoteMediaStream, StableMuteState,
    },
    utils::{yield_now, Callback0, Callback1, HandlerDetachedError},
};

/// Connections service.
// TODO: Store MemberId's or some other metadata, that will make it possible
//       to identify remote Member.
#[derive(Default)]
pub struct Connections {
    /// Local [`PeerId`] to remote [`PeerId`].
    local_to_remote: RefCell<HashMap<PeerId, PeerId>>,

    /// Remote [`PeerId`] to [`Connection`] with that `Peer`.
    connections: RefCell<HashMap<PeerId, Connection>>,

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

    /// Creates new [`Connection`]s based on senders and receivers of provided
    /// [`Track`]s.
    // TODO: creates connections based on remote peer_ids atm, should create
    //       connections based on remote member_ids
    pub fn create_connections_from_tracks<T>(
        &self,
        peer_id: PeerId,
        local_peer: &T,
        tracks: &[Track],
    ) where
        T: MuteStateUpdatesPublisher,
    {
        let create_connection = |connections: &Self, remote_id: &PeerId| {
            let is_new =
                !connections.connections.borrow().contains_key(remote_id);
            if is_new {
                let con = Connection::new(
                    *remote_id,
                    local_peer.on_mute_state_update(),
                );
                connections.on_new_connection.call(con.new_handle());
                connections.connections.borrow_mut().insert(*remote_id, con);
                connections
                    .local_to_remote
                    .borrow_mut()
                    .insert(peer_id, *remote_id);
            }
        };

        for track in tracks {
            match &track.direction {
                Direction::Send { ref receivers, .. } => {
                    for receiver in receivers {
                        create_connection(self, receiver);
                    }
                }
                Direction::Recv { ref sender, .. } => {
                    create_connection(self, sender);
                }
            }
        }
    }

    /// Lookups [`Connection`] by the given remote [`PeerId`].
    pub fn get(&self, remote_peer_id: PeerId) -> Option<Connection> {
        self.connections.borrow().get(&remote_peer_id).cloned()
    }

    /// Closes [`Connection`] associated with provided local [`PeerId`].
    ///
    /// Invokes `on_close` callback.
    pub fn close_connection(&self, local_peer: PeerId) {
        if let Some(remote_id) =
            self.local_to_remote.borrow_mut().remove(&local_peer)
        {
            if let Some(connection) =
                self.connections.borrow_mut().remove(&remote_id)
            {
                // `on_close` callback is invoked here and not in `Drop`
                // implementation so `ConnectionHandle` is
                // available during callback invocation
                connection.0.on_close.call();
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
/// Shared between JS side ([`ConnectionHandle`]) and
/// Rust side ([`Connection`]).
struct InnerConnection {
    /// Remote [`PeerId`].
    remote_id: PeerId,

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
    /// Sets callback, which will be invoked on remote `Member` media stream
    /// arrival.
    pub fn on_remote_stream(&self, f: js_sys::Function) -> Result<(), JsValue> {
        upgrade_or_detached!(self.0)
            .map(|inner| inner.on_remote_stream.set_func(f))
    }

    /// Sets callback, which will be invoked when this `Connection` will close.
    pub fn on_close(&self, f: js_sys::Function) -> Result<(), JsValue> {
        upgrade_or_detached!(self.0).map(|inner| inner.on_close.set_func(f))
    }

    /// Returns remote `PeerId`.
    pub fn get_remote_id(&self) -> Result<u32, JsValue> {
        upgrade_or_detached!(self.0).map(|inner| inner.remote_id.0)
    }
}

/// Connection with a specific remote [`Member`], that is used on Rust side.
///
/// Actually, represents a handle to [`InnerConnection`].
#[derive(Clone)]
pub struct Connection(Rc<InnerConnection>);

impl Connection {
    /// Instantiates new [`Connection`] for a given [`Member`].
    ///
    /// Spawns [`Future`] which will poll provided [`LocalBoxStream`] and notify
    /// [`RemoteMediaStream`] about [`StableMuteState`] changes.
    #[inline]
    pub(crate) fn new(
        remote_id: PeerId,
        mut mute_stream: LocalBoxStream<'static, MuteStateUpdate>,
    ) -> Self {
        let inner = Rc::new(InnerConnection {
            remote_id,
            remote_stream: RefCell::new(None),
            on_remote_stream: Callback1::default(),
            on_close: Callback0::default(),
        });
        let weak_inner = Rc::downgrade(&inner);

        spawn_local(async move {
            while let Some(mute_state_update) = mute_stream.next().await {
                loop {
                    let is_finished = async {
                        yield_now().await;
                        let inner = if let Some(inner) = weak_inner.upgrade() {
                            inner
                        } else {
                            return false;
                        };
                        let stream = inner.remote_stream.borrow();
                        if let Some(stream) = stream.as_ref() {
                            match mute_state_update.new_mute_state {
                                StableMuteState::Muted => {
                                    stream
                                        .track_stopped(mute_state_update.kind);
                                }
                                StableMuteState::NotMuted => {
                                    stream
                                        .track_started(mute_state_update.kind);
                                }
                            }
                        } else {
                            return false;
                        }

                        true
                    };

                    if is_finished.await {
                        break;
                    }
                }
            }
        });

        Self(inner)
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

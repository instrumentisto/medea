//! Connection with specific remote `Member`.

use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use medea_client_api_proto::TrackId;
use wasm_bindgen::prelude::*;

use crate::{
    media::{MediaStreamTrack, TrackKind},
    peer::{PeerMediaStream, RemoteMediaStream, StableMuteState},
    utils::{console_error, Callback, HandlerDetachedError},
};
use futures::{
    stream::{BoxStream, LocalBoxStream},
    StreamExt,
};
use wasm_bindgen_futures::spawn_local;

/// Actual data of a connection with a specific remote [`Member`].
///
/// Shared between JS side ([`ConnectionHandle`]) and
/// Rust side ([`Connection`]).
struct InnerConnection {
    remote_stream: RefCell<Option<PeerMediaStream>>,
    on_remote_stream: Callback<RemoteMediaStream>,
}

/// Connection with a specific remote `Member`, that is used on JS side.
///
/// Actually, represents a [`Weak`]-based handle to `InnerConnection`.
#[wasm_bindgen]
pub struct ConnectionHandle(Weak<InnerConnection>);

#[wasm_bindgen]
impl ConnectionHandle {
    /// Sets callback, which will be invoked on remote `Member` media stream
    /// arrival.
    pub fn on_remote_stream(&self, f: js_sys::Function) -> Result<(), JsValue> {
        upgrade_or_detached!(self.0)
            .map(|inner| inner.on_remote_stream.set_func(f))
    }
}

/// Connection with a specific remote [`Member`], that is used on Rust side.
///
/// Actually, represents a handle to [`InnerConnection`].
pub(crate) struct Connection(Rc<InnerConnection>);

impl Connection {
    /// Instantiates new [`Connection`] for a given [`Member`].
    #[inline]
    pub(crate) fn new(
        mut mute_stream: LocalBoxStream<'static, (TrackKind, StableMuteState)>,
    ) -> Self {
        let inner = Rc::new(InnerConnection {
            remote_stream: RefCell::new(None),
            on_remote_stream: Callback::default(),
        });
        let weak_inner = Rc::downgrade(&inner);

        spawn_local(async move {
            while let Some((kind, mute_state)) = mute_stream.next().await {
                let inner = if let Some(inner) = weak_inner.upgrade() {
                    inner
                } else {
                    break;
                };
                let stream = inner.remote_stream.borrow();
                if let Some(stream) = stream.as_ref() {
                    match mute_state {
                        StableMuteState::Muted => {
                            stream.track_stopped(kind);
                        }
                        StableMuteState::NotMuted => {
                            stream.track_started(kind);
                        }
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
    pub(crate) fn add_remote_track(
        &self,
        track_id: TrackId,
        track: MediaStreamTrack,
    ) {
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
    pub(crate) fn new_handle(&self) -> ConnectionHandle {
        ConnectionHandle(Rc::downgrade(&self.0))
    }
}

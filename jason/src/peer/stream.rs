//! [MediaStream][1] related objects.
//!
//! [1]: https://w3.org/TR/mediacapture-streams/#mediastream

use std::{
    cell::RefCell,
    collections::HashMap,
    rc::{Rc, Weak},
};

use medea_client_api_proto::TrackId;
use wasm_bindgen::{prelude::*, JsValue};
use web_sys::{
    MediaStream as SysMediaStream, MediaStreamTrack as SysMediaStreamTrack,
};

use crate::{
    media::{MediaStreamTrack, TrackKind},
    utils::{Callback1, HandlerDetachedError},
};

/// Actual data of a [`PeerMediaStream`].
///
/// Shared between JS side ([`RemoteMediaStream`]) and Rust side
/// ([`PeerMediaStream`]).
struct InnerStream {
    /// Actual underlying [MediaStream][1] object.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastream
    stream: SysMediaStream,

    /// List of audio tracks.
    audio_tracks: RefCell<HashMap<TrackId, MediaStreamTrack>>,

    /// List of video tracks.
    video_tracks: RefCell<HashMap<TrackId, MediaStreamTrack>>,

    /// Callback from JS side which will be invoked on new `MediaTrack` adding.
    on_track_added: Callback1<SysMediaStreamTrack>,

    /// Callback from JS side which will be invoked on `MediaTrack` starting.
    on_track_enabled: Callback1<SysMediaStreamTrack>,

    /// Callback from JS side which will be invoked on `MediaTrack` stopping.
    on_track_disabled: Callback1<SysMediaStreamTrack>,
}

impl InnerStream {
    /// Instantiates new [`InnerStream`].
    fn new() -> Self {
        Self {
            stream: SysMediaStream::new().unwrap(),
            audio_tracks: RefCell::default(),
            video_tracks: RefCell::default(),
            on_track_added: Callback1::default(),
            on_track_enabled: Callback1::default(),
            on_track_disabled: Callback1::default(),
        }
    }

    /// Adds provided [`MediaStreamTrack`] to a stream.
    fn add_track(&self, track_id: TrackId, track: MediaStreamTrack) {
        self.stream.add_track(track.as_ref());

        let track_kind = track.kind();
        let sys_track = track.as_sys();
        match track_kind {
            TrackKind::Audio => {
                self.audio_tracks.borrow_mut().insert(track_id, track);
            }
            TrackKind::Video => {
                self.video_tracks.borrow_mut().insert(track_id, track);
            }
        };
        self.on_track_added.call(sys_track);
    }
}

/// Representation of [MediaStream][1] object. Each of its tracks has
/// association with [`TrackId`].
///
/// It's used on Rust side and represents a handle to [`InnerStream`] data.
///
/// For using [`PeerMediaStream`] on JS side, consider the
/// [`RemoteMediaStream`].
///
/// [1]: https://w3.org/TR/mediacapture-streams/#mediastream
#[derive(Clone)]
pub struct PeerMediaStream(Rc<InnerStream>);

#[allow(clippy::new_without_default)]
impl PeerMediaStream {
    /// Creates empty [`PeerMediaStream`].
    pub fn new() -> Self {
        Self(Rc::new(InnerStream::new()))
    }

    /// Adds provided [`MediaStreamTrack`] to a stream.
    pub fn add_track(&self, track_id: TrackId, track: MediaStreamTrack) {
        self.0.add_track(track_id, track);
    }

    /// Checks if [`PeerMediaStream`] contains a [`MediaStreamTrack`] with
    /// specified ID.
    pub fn has_track(&self, id: TrackId) -> bool {
        self.0.video_tracks.borrow().contains_key(&id)
            || self.0.audio_tracks.borrow().contains_key(&id)
    }

    /// Returns a [`MediaStreamTrack`] of [`PeerMediaStream`] by its ID, if any.
    pub fn get_track_by_id(
        &self,
        track_id: TrackId,
    ) -> Option<MediaStreamTrack> {
        match self.0.video_tracks.borrow().get(&track_id) {
            Some(track) => Some(track.clone()),
            None => match self.0.audio_tracks.borrow().get(&track_id) {
                Some(track) => Some(track.clone()),
                None => None,
            },
        }
    }

    /// Instantiates new [`RemoteMediaStream`] for use on JS side.
    pub fn new_handle(&self) -> RemoteMediaStream {
        RemoteMediaStream(Rc::downgrade(&self.0))
    }

    /// Returns actual underlying [MediaStream][1] object.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastream
    pub fn stream(&self) -> SysMediaStream {
        Clone::clone(&self.0.stream)
    }

    /// Notifies [`PeerMediaStream`] that `MediaTrack` with provided
    /// [`TrackKind`] was started.
    ///
    /// Calls [`PeerMediaStream::on_track_enabled`] JS callback function.
    pub fn track_started(&self, track: &MediaStreamTrack) {
        self.0.on_track_enabled.call(track.as_sys());
    }

    /// Notifies [`PeerMediaStream`] that `MediaTrack` with provided
    /// [`TrackKind`] was stopped.
    ///
    /// Calls [`PeerMediaStream::on_track_disabled`] JS callback function.
    pub fn track_stopped(&self, track: &MediaStreamTrack) {
        self.0.on_track_disabled.call(track.as_sys());
    }
}

/// JS side handle to [`PeerMediaStream`].
///
/// Actually, represents a [`Weak`]-based handle to `InnerStream`.
///
/// For using [`RemoteMediaStream`] on Rust side, consider the
/// [`PeerMediaStream`].
#[wasm_bindgen]
pub struct RemoteMediaStream(Weak<InnerStream>);

#[wasm_bindgen]
impl RemoteMediaStream {
    /// Returns the underlying [`PeerMediaStream`][`SysMediaStream`] object.
    pub fn get_media_stream(&self) -> Result<SysMediaStream, JsValue> {
        upgrade_or_detached!(self.0).map(|inner| Clone::clone(&inner.stream))
    }

    /// Returns `true` if at least one video [`MediaStreamTrack`] exists in this
    /// [`RemoteMediaStream`].
    pub fn has_active_audio(&self) -> Result<bool, JsValue> {
        upgrade_or_detached!(self.0).map(|inner| {
            for audio_track in inner.audio_tracks.borrow().values() {
                if audio_track.is_active() {
                    return true;
                }
            }

            false
        })
    }

    /// Returns `true` if at least one video [`MediaStreamTrack`] exists in this
    /// [`RemoteMediaStream`].
    pub fn has_active_video(&self) -> Result<bool, JsValue> {
        upgrade_or_detached!(self.0).map(|inner| {
            for video_track in inner.video_tracks.borrow().values() {
                if video_track.is_active() {
                    return true;
                }
            }

            false
        })
    }

    /// Sets callback, which will be invoked on new `MediaTrack` adding.
    pub fn on_track_added(&self, f: js_sys::Function) -> Result<(), JsValue> {
        upgrade_or_detached!(self.0).map(|inner| {
            inner.on_track_added.set_func(f);
            {
                inner
                    .audio_tracks
                    .borrow()
                    .values()
                    .chain(inner.video_tracks.borrow().values())
                    .for_each(|track| {
                        inner.on_track_added.call(track.as_sys());
                    });
            }
        })
    }

    /// Sets callback, which will be invoked on `MediaTrack` starting.
    pub fn on_track_enabled(&self, f: js_sys::Function) -> Result<(), JsValue> {
        upgrade_or_detached!(self.0).map(|inner| {
            inner.on_track_enabled.set_func(f);
        })
    }

    /// Sets callback, which will be invoked on `MediaTrack` stopping.
    pub fn on_track_disabled(
        &self,
        f: js_sys::Function,
    ) -> Result<(), JsValue> {
        upgrade_or_detached!(self.0).map(|inner| {
            inner.on_track_disabled.set_func(f);
        })
    }
}

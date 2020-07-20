//! [MediaStream][1] related objects.
//!
//! [1]: https://w3.org/TR/mediacapture-streams/#mediastream

use std::{
    cell::RefCell,
    collections::HashMap,
    rc::{Rc, Weak},
};

use futures::StreamExt;
use medea_client_api_proto::TrackId;
use wasm_bindgen::{prelude::*, JsValue};
use wasm_bindgen_futures::spawn_local;
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

    /// Callback from JS side which will be invoked on `MediaTrack` enabling.
    on_track_enabled: Rc<Callback1<SysMediaStreamTrack>>,

    /// Callback from JS side which will be invoked on `MediaTrack` disabling.
    on_track_disabled: Rc<Callback1<SysMediaStreamTrack>>,
}

impl InnerStream {
    /// Instantiates new [`InnerStream`].
    fn new() -> Self {
        Self {
            stream: SysMediaStream::new().unwrap(),
            audio_tracks: RefCell::default(),
            video_tracks: RefCell::default(),
            on_track_added: Callback1::default(),
            on_track_enabled: Rc::new(Callback1::default()),
            on_track_disabled: Rc::new(Callback1::default()),
        }
    }

    /// Adds provided [`MediaStreamTrack`] to a stream.
    fn add_track(&self, track_id: TrackId, track: MediaStreamTrack) {
        self.stream.add_track(track.as_ref());

        let mut track_enabled_state_changes =
            track.enabled().subscribe().skip(1);
        let sys_track = Clone::clone(track.as_ref());
        let weak_track = track.downgrade();
        match track.kind() {
            TrackKind::Audio => {
                self.audio_tracks.borrow_mut().insert(track_id, track);
            }
            TrackKind::Video => {
                self.video_tracks.borrow_mut().insert(track_id, track);
            }
        };

        self.on_track_added.call(sys_track);

        let on_track_enabled = Rc::clone(&self.on_track_enabled);
        let on_track_disabled = Rc::clone(&self.on_track_disabled);
        spawn_local(async move {
            while let Some(enabled) = track_enabled_state_changes.next().await {
                if let Some(track) = weak_track.upgrade() {
                    if enabled {
                        on_track_enabled.call(Clone::clone(track.as_ref()));
                    } else {
                        on_track_disabled.call(Clone::clone(track.as_ref()));
                    }
                } else {
                    break;
                }
            }
        })
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
                if audio_track.enabled().get() {
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
                if video_track.enabled().get() {
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
            inner
                .audio_tracks
                .borrow()
                .values()
                .chain(inner.video_tracks.borrow().values())
                .for_each(|track| {
                    inner.on_track_added.call(Clone::clone(track.as_ref()));
                });
        })
    }

    /// Sets callback, which will be invoked on `MediaTrack` enabling.
    pub fn on_track_enabled(&self, f: js_sys::Function) -> Result<(), JsValue> {
        upgrade_or_detached!(self.0).map(|inner| {
            inner.on_track_enabled.set_func(f);
        })
    }

    /// Sets callback, which will be invoked on `MediaTrack` disabling.
    pub fn on_track_disabled(
        &self,
        f: js_sys::Function,
    ) -> Result<(), JsValue> {
        upgrade_or_detached!(self.0).map(|inner| {
            inner.on_track_disabled.set_func(f);
        })
    }
}

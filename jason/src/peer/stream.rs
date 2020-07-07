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
use web_sys::MediaStream as SysMediaStream;

use crate::{
    media::{MediaStreamTrack, TrackKind},
    utils::{console_error, Callback, EventListener, HandlerDetachedError},
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
    audio_tracks: HashMap<TrackId, MediaStreamTrack>,

    /// List of video tracks.
    video_tracks: HashMap<TrackId, MediaStreamTrack>,

    on_track_added: Callback<OnTrackAddedMetadata>,

    on_track_stopped: Callback<OnTrackRemovedMetadata>,
}

#[wasm_bindgen]
struct OnTrackRemovedMetadata {
    kind: TrackKind,
    track_id: TrackId,
    is_last: bool,
}

#[wasm_bindgen]
impl OnTrackRemovedMetadata {
    pub fn kind(&self) -> String {
        self.kind.to_string()
    }

    pub fn track_id(&self) -> u32 {
        self.track_id.0
    }

    pub fn is_last(&self) -> bool {
        self.is_last
    }
}

#[wasm_bindgen]
struct OnTrackAddedMetadata {
    kind: TrackKind,
    track_id: TrackId,
    is_first: bool,
}

#[wasm_bindgen]
impl OnTrackAddedMetadata {
    pub fn kind(&self) -> String {
        self.kind.to_string()
    }

    pub fn is_audio(&self) -> bool {
        self.kind == TrackKind::Audio
    }

    pub fn is_video(&self) -> bool {
        self.kind == TrackKind::Video
    }

    pub fn track_id(&self) -> u32 {
        self.track_id.0
    }

    pub fn is_first(&self) -> bool {
        self.is_first
    }
}

impl InnerStream {
    /// Instantiates new [`InnerStream`].
    fn new() -> Self {
        Self {
            stream: SysMediaStream::new().unwrap(),
            audio_tracks: HashMap::new(),
            video_tracks: HashMap::new(),
            on_track_added: Callback::default(),
            on_track_stopped: Callback::default(),
        }
    }

    /// Adds provided [`MediaStreamTrack`] to a stream.
    fn add_track(&mut self, track_id: TrackId, track: MediaStreamTrack) {
        self.stream.add_track(track.as_ref());

        let is_first;
        let track_kind = track.kind();
        match track.kind() {
            TrackKind::Audio => {
                is_first = self.audio_tracks.is_empty();
                self.audio_tracks.insert(track_id, track);
            }
            TrackKind::Video => {
                is_first = self.video_tracks.is_empty();
                self.video_tracks.insert(track_id, track);
            }
        };

        let on_track_added_metadata = OnTrackAddedMetadata {
            track_id,
            kind: track_kind,
            is_first,
        };
        self.on_track_added.call(on_track_added_metadata);
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
pub struct PeerMediaStream(Rc<RefCell<InnerStream>>);

#[allow(clippy::new_without_default)]
impl PeerMediaStream {
    /// Creates empty [`PeerMediaStream`].
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(InnerStream::new())))
    }

    /// Adds provided [`MediaStreamTrack`] to a stream.
    pub fn add_track(&self, track_id: TrackId, track: MediaStreamTrack) {
        self.0.borrow_mut().add_track(track_id, track);
    }

    /// Checks if [`PeerMediaStream`] contains a [`MediaStreamTrack`] with
    /// specified ID.
    pub fn has_track(&self, id: TrackId) -> bool {
        let inner = self.0.borrow();
        inner.video_tracks.contains_key(&id)
            || inner.audio_tracks.contains_key(&id)
    }

    /// Returns a [`MediaStreamTrack`] of [`PeerMediaStream`] by its ID, if any.
    pub fn get_track_by_id(
        &self,
        track_id: TrackId,
    ) -> Option<MediaStreamTrack> {
        let inner = self.0.borrow();
        match inner.video_tracks.get(&track_id) {
            Some(track) => Some(track.clone()),
            None => match inner.audio_tracks.get(&track_id) {
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
        Clone::clone(&self.0.borrow().stream)
    }
}

/// JS side handle to [`PeerMediaStream`].
///
/// Actually, represents a [`Weak`]-based handle to `InnerStream`.
///
/// For using [`RemoteMediaStream`] on Rust side, consider the
/// [`PeerMediaStream`].
#[wasm_bindgen]
pub struct RemoteMediaStream(Weak<RefCell<InnerStream>>);

#[wasm_bindgen]
impl RemoteMediaStream {
    /// Returns the underlying [`PeerMediaStream`][`SysMediaStream`] object.
    pub fn get_media_stream(&self) -> Result<SysMediaStream, JsValue> {
        upgrade_or_detached!(self.0)
            .map(|inner| Clone::clone(&inner.borrow().stream))
    }

    pub fn on_track_added(&self, f: js_sys::Function) -> Result<(), JsValue> {
        upgrade_or_detached!(self.0).map(|inner| {
            inner.borrow().on_track_added.set_func(f);
            {
                let inner = inner.borrow();
                inner
                    .audio_tracks
                    .iter()
                    .enumerate()
                    .chain(inner.video_tracks.iter().enumerate())
                    .for_each(|(num, (track_id, track))| {
                        let is_first = num == 0;
                        inner.on_track_added.call(OnTrackAddedMetadata {
                            track_id: *track_id,
                            kind: track.kind(),
                            is_first,
                        });
                    });
            }
        })
    }

    pub fn on_track_stopped(&self, f: js_sys::Function) -> Result<(), JsValue> {
        upgrade_or_detached!(self.0).map(|inner| {
            inner.borrow().on_track_stopped.set_func(f);
        })
    }
}

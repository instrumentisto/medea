//! [MediaStream][1] related objects.
//!
//! [1]: https://www.w3.org/TR/mediacapture-streams/#mediastream

use std::{
    collections::HashMap,
    rc::{Rc, Weak},
};

use medea_client_api_proto::MediaType;
use wasm_bindgen::{prelude::*, JsValue};
use web_sys::MediaStream as SysMediaStream;

use crate::utils::WasmErr;

use super::{MediaTrack, TrackId};

/// Actual data of a [`MediaStream`].
///
/// Shared between JS side ([`MediaStreamHandle`]) and Rust side
/// ([`MediaStream`]).
#[derive(Debug)]
struct InnerStream {
    /// Actual underlying [MediaStream][1] object.
    ///
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastream
    stream: SysMediaStream,

    /// List of audio tracks.
    audio_tracks: HashMap<u64, Rc<MediaTrack>>,

    /// List of video tracks.
    video_tracks: HashMap<u64, Rc<MediaTrack>>,
}

impl InnerStream {
    /// Instantiates new [`InnerStream`].
    fn new() -> Self {
        Self {
            stream: SysMediaStream::new().unwrap(),
            audio_tracks: HashMap::new(),
            video_tracks: HashMap::new(),
        }
    }

    /// Adds provided [`MediaTrack`] to a stream.
    fn add_track(&mut self, track: Rc<MediaTrack>) {
        self.stream.add_track(track.track());
        let caps = track.caps();
        match caps {
            MediaType::Audio(_) => {
                self.audio_tracks.insert(track.id(), track);
            }
            MediaType::Video(_) => {
                self.video_tracks.insert(track.id(), track);
            }
        }
    }
}

/// Representation of [MediaStream][1] object.
///
/// It's used on Rust side and represents a handle to [`InnerStream`] data.
///
/// For using [`MediaStream`] on JS side, consider the [`MediaStreamHandle`].
///
/// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastream
#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
pub struct MediaStream(Rc<InnerStream>);

impl MediaStream {
    /// Creates new [`MediaStream`] from a given collection of [`MediaTrack`]s.
    pub fn from_tracks<I>(tracks: I) -> Self
    where
        I: IntoIterator<Item = Rc<MediaTrack>>,
    {
        let mut stream = InnerStream::new();
        for track in tracks {
            stream.add_track(track);
        }
        Self(Rc::new(stream))
    }

    /// Checks if [`MediaStream`] contains a [`MediaTrack`] with specified ID.
    pub fn has_track(&self, id: TrackId) -> bool {
        self.0.video_tracks.contains_key(&id)
            || self.0.audio_tracks.contains_key(&id)
    }

    /// Returns a [`MediaTrack`] of [`MediaStream`] by its ID, if any.
    pub fn get_track_by_id(&self, track_id: TrackId) -> Option<Rc<MediaTrack>> {
        match self.0.video_tracks.get(&track_id) {
            Some(track) => Some(Rc::clone(track)),
            None => match self.0.audio_tracks.get(&track_id) {
                Some(track) => Some(Rc::clone(track)),
                None => None,
            },
        }
    }

    /// Instantiates new [`MediaStreamHandle`] for use on JS side.
    pub fn new_handle(&self) -> MediaStreamHandle {
        MediaStreamHandle(Rc::downgrade(&self.0))
    }
}

/// JS side handle to [`MediaStream`].
///
/// Actually, represents a [`Weak`]-based handle to [`InnerStream`].
///
/// For using [`MediaStreamHandle`] on Rust side, consider the [`MediaStream`].
#[wasm_bindgen]
#[derive(Debug)]
pub struct MediaStreamHandle(Weak<InnerStream>);

#[wasm_bindgen]
impl MediaStreamHandle {
    /// Returns the underlying [`MediaStream`][`SysMediaStream`] object.
    pub fn get_media_stream(&self) -> Result<SysMediaStream, JsValue> {
        match self.0.upgrade() {
            Some(inner) => Ok(inner.stream.clone()),
            None => Err(WasmErr::from("Detached state").into()),
        }
    }
}

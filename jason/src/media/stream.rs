//! Wrappers and adapters for [`MediaStream`][1] and relate objects.
//!
//! [1]: https://www.w3.org/TR/mediacapture-streams/#mediastream

use std::collections::HashMap;
use std::rc::{Rc, Weak};

use medea_client_api_proto::MediaType;
use wasm_bindgen::{prelude::*, JsValue};
use web_sys::MediaStream as SysMediaStream;

use crate::{
    media::{MediaTrack, TrackId},
    utils::WasmErr,
};

/// [`MediaStream`] object wrapper.
///
/// Shared between JS-side handle ([`MediaStreamHandle`])
/// and Rust-side handle ([`MediaStream`]).
struct InnerStream {
    /// Actual [`MediaStream`][1] object.
    ///
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastream
    stream: SysMediaStream,

    /// List of [`MediaStream`]s audio tracks.
    audio_tracks: HashMap<u64, Rc<MediaTrack>>,

    /// List of [`MediaStream`]s video tracks.
    video_tracks: HashMap<u64, Rc<MediaTrack>>,
}

impl InnerStream {
    fn new() -> Self {
        Self {
            stream: SysMediaStream::new().unwrap(),
            audio_tracks: HashMap::new(),
            video_tracks: HashMap::new(),
        }
    }

    /// Adds provided track.
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

/// Rust-side [`MediaStream`] handle.
#[allow(clippy::module_name_repetitions)]
pub struct MediaStream(Rc<InnerStream>);

impl MediaStream {
    pub fn from_tracks(tracks: Vec<Rc<MediaTrack>>) -> Self {
        let mut stream = InnerStream::new();

        for track in tracks {
            stream.add_track(track);
        }

        Self(Rc::new(stream))
    }

    pub fn has_track(&self, track_id: TrackId) -> bool {
        self.0.video_tracks.contains_key(&track_id)
            || self.0.audio_tracks.contains_key(&track_id)
    }

    pub fn get_track_by_id(&self, track_id: TrackId) -> Option<Rc<MediaTrack>> {
        match self.0.video_tracks.get(&track_id) {
            Some(track) => Some(Rc::clone(track)),
            None => match self.0.audio_tracks.get(&track_id) {
                Some(track) => Some(Rc::clone(track)),
                None => None,
            },
        }
    }

    pub fn new_handle(&self) -> MediaStreamHandle {
        MediaStreamHandle(Rc::downgrade(&self.0))
    }
}

/// JS-side [`MediaStream`] handle.
#[wasm_bindgen]
pub struct MediaStreamHandle(Weak<InnerStream>);

#[wasm_bindgen]
impl MediaStreamHandle {
    pub fn get_media_stream(&self) -> Result<SysMediaStream, JsValue> {
        match self.0.upgrade() {
            Some(inner) => Ok(inner.stream.clone()),
            None => Err(WasmErr::build_from_str("Detached state").into()),
        }
    }
}

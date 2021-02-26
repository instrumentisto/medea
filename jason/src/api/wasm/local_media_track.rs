use derive_more::From;

use wasm_bindgen::prelude::*;

use crate::{
    api::{MediaKind, MediaSourceKind},
    core,
};

/// Wrapper around local [MediaStreamTrack][1].
///
/// Backed by strong reference to actual track that implements auto stop on
/// drop. Can be manually dropped with `free()` call.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack
#[wasm_bindgen]
#[derive(From)]
pub struct LocalMediaTrack(core::LocalMediaTrack);

#[wasm_bindgen]
impl LocalMediaTrack {
    /// Returns the underlying [MediaStreamTrack][1].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack
    pub fn get_track(&self) -> web_sys::MediaStreamTrack {
        Clone::clone(&self.0.get_track().as_ref())
    }

    /// Returns [`MediaKind::Audio`] if this [`LocalMediaTrack`] represents an
    /// audio track, or [`MediaKind::Video`] if it represents a video track.
    pub fn kind(&self) -> MediaKind {
        self.0.kind().into()
    }

    /// Returns [`MediaSourceKind::Device`] if this [`LocalMediaTrack`] is
    /// sourced from some device (webcam/microphone), or
    /// [`MediaSourceKind::Display`] if it is captured via
    /// [MediaDevices.getDisplayMedia()][1].
    ///
    /// [1]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
    pub fn media_source_kind(&self) -> MediaSourceKind {
        self.0.media_source_kind().into()
    }
}

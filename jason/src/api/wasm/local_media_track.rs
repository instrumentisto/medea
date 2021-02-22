use derive_more::From;

use wasm_bindgen::prelude::*;

use crate::core;

/// Wrapper around strongly referenced [`Track`] for JS side.
#[wasm_bindgen]
#[derive(From)]
pub struct LocalMediaTrack(core::LocalMediaTrack);

#[wasm_bindgen]
impl LocalMediaTrack {
    /// Returns the underlying [`sys::MediaStreamTrack`] of this [`JsTrack`].
    pub fn get_track(&self) -> web_sys::MediaStreamTrack {
        Clone::clone(&self.0.get_track().as_ref())
    }

    /// Returns [`MediaKind::Audio`] if this [`JsTrack`] represents an audio
    /// track, or [`MediaKind::Video`] if it represents a video track.
    pub fn kind(&self) -> super::MediaKind {
        self.0.kind().into()
    }

    /// Returns [`JsMediaSourceKind::Device`] if this [`JsTrack`] is sourced
    /// from some device (webcam/microphone), or [`JsMediaSourceKind::Display`]
    /// if ot is captured via [MediaDevices.getDisplayMedia()][1].
    ///
    /// [1]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
    pub fn media_source_kind(&self) -> super::MediaSourceKind {
        self.0.media_source_kind().into()
    }
}

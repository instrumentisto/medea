use derive_more::{From, Into};
use wasm_bindgen::prelude::*;

use crate::{
    api::{MediaKind, MediaSourceKind},
    core,
};

/// Wrapper around [MediaStreamTrack][1] received from the remote.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack
#[wasm_bindgen]
#[derive(Clone, From, Into)]
pub struct RemoteMediaTrack(core::remote::Track);

#[wasm_bindgen]
impl RemoteMediaTrack {
    /// Returns the underlying [MediaStreamTrack][1] of this
    /// [`RemoteMediaTrack`].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack
    pub fn get_track(&self) -> web_sys::MediaStreamTrack {
        Clone::clone(self.0.get_track().as_ref())
    }

    /// Indicate whether this [`RemoteMediaTrack`] is enabled.
    pub fn enabled(&self) -> bool {
        self.0.enabled()
    }

    /// Sets callback to invoke when this [`RemoteMediaTrack`] is enabled.
    pub fn on_enabled(&self, cb: js_sys::Function) {
        self.0.on_enabled(cb.into())
    }

    /// Sets callback to invoke when this [`RemoteMediaTrack`] is disabled.
    pub fn on_disabled(&self, cb: js_sys::Function) {
        self.0.on_disabled(cb.into())
    }

    /// Returns [`MediaKind::Audio`] if this [`RemoteMediaTrack`] represents an
    /// audio track, or [`MediaKind::Video`] if it represents a video track.
    pub fn kind(&self) -> MediaKind {
        self.0.kind().into()
    }

    /// Returns [`MediaSourceKind::Device`] if this [`RemoteMediaTrack`] is
    /// sourced from some device (webcam/microphone), or
    /// [`MediaSourceKind::Display`] if it's captured via
    /// [MediaDevices.getDisplayMedia()][1].
    ///
    /// [1]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
    pub fn media_source_kind(&self) -> MediaSourceKind {
        self.0.media_source_kind().into()
    }
}

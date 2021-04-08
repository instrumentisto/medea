//! Wrapper around a received remote [MediaStreamTrack][1].
//!
//! [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack

use derive_more::{From, Into};
use wasm_bindgen::prelude::*;

use crate::{
    api::{MediaKind, MediaSourceKind},
    media::track::remote,
};

/// Wrapper around a received remote [MediaStreamTrack][1].
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack
#[wasm_bindgen]
#[derive(Clone, From, Into)]
pub struct RemoteMediaTrack(remote::Track);

#[wasm_bindgen]
impl RemoteMediaTrack {
    /// Returns the underlying [MediaStreamTrack][1].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack
    #[must_use]
    pub fn get_track(&self) -> web_sys::MediaStreamTrack {
        Clone::clone(self.0.get_track().as_ref())
    }

    /// Indicates whether this [`RemoteMediaTrack`] is enabled.
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.0.enabled()
    }

    /// Sets callback, invoked when this [`RemoteMediaTrack`] is enabled.
    pub fn on_enabled(&self, cb: js_sys::Function) {
        self.0.on_enabled(cb.into())
    }

    /// Sets callback, invoked when this [`RemoteMediaTrack`] is disabled.
    pub fn on_disabled(&self, cb: js_sys::Function) {
        self.0.on_disabled(cb.into())
    }

    /// Returns a [`MediaKind::Audio`] if this [`RemoteMediaTrack`] represents
    /// an audio track, or a [`MediaKind::Video`] if it represents a video
    /// track.
    #[must_use]
    pub fn kind(&self) -> MediaKind {
        self.0.kind().into()
    }

    /// Returns a [`MediaSourceKind::Device`] if this [`RemoteMediaTrack`] is
    /// sourced from some device (webcam/microphone), or a
    /// [`MediaSourceKind::Display`] if it's captured via
    /// [MediaDevices.getDisplayMedia()][1].
    ///
    /// [1]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
    #[must_use]
    pub fn media_source_kind(&self) -> MediaSourceKind {
        self.0.media_source_kind().into()
    }
}

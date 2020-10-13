//! Adapters to [Media Capture and Streams API][1].
//!
//! [1]: https://w3.org/TR/mediacapture-streams

mod constraints;
mod device_info;
mod manager;
mod track;

use wasm_bindgen::prelude::*;

#[doc(inline)]
pub use self::{
    constraints::{
        AudioTrackConstraints, DeviceVideoTrackConstraints,
        DisplayVideoTrackConstraints, FacingMode, LocalTracksConstraints,
        MediaStreamSettings, MultiSourceTracksConstraints, RecvConstraints,
        TrackConstraints, VideoSource,
    },
    device_info::InputDeviceInfo,
    manager::{MediaManager, MediaManagerError, MediaManagerHandle},
    track::{JsMediaSourceKind, MediaStreamTrack},
};

/// [MediaStreamTrack.kind][1] representation.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack-kind
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaKind {
    /// Audio track.
    Audio,

    /// Video track.
    Video,
}

impl MediaKind {
    /// Returns string representation of a [`MediaKind`].
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Audio => "audio",
            Self::Video => "video",
        }
    }
}

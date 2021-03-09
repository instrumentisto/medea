//! Adapters to [Media Capture and Streams API][1].
//!
//! [1]: https://w3.org/TR/mediacapture-streams

mod constraints;
mod device_info;
mod manager;
pub mod track;

use derive_more::Display;
use wasm_bindgen::prelude::*;

#[doc(inline)]
pub use self::{
    constraints::{
        AudioMediaTracksSettings, AudioTrackConstraints,
        DeviceVideoTrackConstraints, DisplayVideoTrackConstraints, FacingMode,
        LocalTracksConstraints, MediaStreamSettings,
        MultiSourceTracksConstraints, RecvConstraints, TrackConstraints,
        VideoSource, VideoTrackConstraints,
    },
    device_info::InputDeviceInfo,
    manager::{MediaManager, MediaManagerError, MediaManagerHandle},
    track::JsMediaSourceKind,
};

/// [MediaStreamTrack.kind][1] representation.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack-kind
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, Display, Eq, PartialEq)]
pub enum MediaKind {
    /// Audio track.
    #[display(fmt = "audio")]
    Audio,

    /// Video track.
    #[display(fmt = "video")]
    Video,
}

impl MediaKind {
    /// Returns string representation of a [`MediaKind`].
    #[inline]
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Audio => "audio",
            Self::Video => "video",
        }
    }
}

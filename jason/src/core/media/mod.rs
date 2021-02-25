//! Adapters to [Media Capture and Streams API][1].
//!
//! [1]: https://w3.org/TR/mediacapture-streams

pub mod constraints;
mod manager;
pub mod track;

use derive_more::Display;

#[doc(inline)]
pub use self::{
    constraints::{
        AudioMediaTracksSettings, AudioTrackConstraints,
        DeviceVideoTrackConstraints, DisplayVideoTrackConstraints,
        LocalTracksConstraints, MediaStreamSettings,
        MultiSourceTracksConstraints, RecvConstraints, TrackConstraints,
        VideoSource, VideoTrackConstraints,
    },
    manager::{MediaManager, MediaManagerError, MediaManagerHandle},
    track::MediaSourceKind,
};

/// [MediaStreamTrack.kind][1] representation.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack-kind
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
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Audio => "audio",
            Self::Video => "video",
        }
    }
}

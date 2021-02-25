use derive_more::Display;
use wasm_bindgen::prelude::*;

pub mod connection_handle;
pub mod constraints_update_exception;
pub mod input_device_info;
pub mod jason;
pub mod jason_error;
pub mod local_media_track;
pub mod media_manager_handle;
pub mod media_stream_settings;
pub mod reconnect_handle;
pub mod remote_media_track;
pub mod room_close_reason;
pub mod room_handle;

use crate::core;

/// [MediaStreamTrack.kind][1] representation.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack-kind
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, Display, Eq, PartialEq)]
pub enum MediaKind {
    /// Audio track.
    Audio,

    /// Video track.
    Video,
}

impl From<core::MediaKind> for MediaKind {
    fn from(that: core::MediaKind) -> Self {
        match that {
            core::MediaKind::Audio => Self::Audio,
            core::MediaKind::Video => Self::Video,
        }
    }
}

#[wasm_bindgen]
#[derive(Clone, Copy, Debug, Display, Eq, PartialEq)]
pub enum MediaSourceKind {
    /// Media is sourced from some media device (webcam or microphone).
    Device,

    /// Media is obtained with screen-capture.
    Display,
}

impl From<core::MediaSourceKind> for MediaSourceKind {
    fn from(that: core::MediaSourceKind) -> Self {
        match that {
            core::MediaSourceKind::Device => Self::Device,
            core::MediaSourceKind::Display => Self::Display,
        }
    }
}

impl Into<core::MediaSourceKind> for MediaSourceKind {
    fn into(self) -> core::MediaSourceKind {
        match self {
            MediaSourceKind::Device => core::MediaSourceKind::Device,
            MediaSourceKind::Display => core::MediaSourceKind::Display,
        }
    }
}

/// Describes the directions that the camera can face, as seen from the user's
/// perspective. Representation of [VideoFacingModeEnum][1].
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-videofacingmodeenum
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, Display, Eq, PartialEq)]
pub enum FacingMode {
    /// Facing toward the user (a self-view camera).
    User,

    /// Facing away from the user (viewing the environment).
    Environment,

    /// Facing to the left of the user.
    Left,

    /// Facing to the right of the user.
    Right,
}

//! External [`Jason`] API for `wasm32-unknown-unknown` target, designed to be
//! used in a web environment with JavaScript.
//!
//! [`Jason`]: crate::api::Jason

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

use derive_more::Display;
use wasm_bindgen::prelude::*;

use crate::media;

pub use self::jason_error::JasonError;

impl From<media::MediaKind> for MediaKind {
    #[inline]
    fn from(that: media::MediaKind) -> Self {
        match that {
            media::MediaKind::Audio => Self::Audio,
            media::MediaKind::Video => Self::Video,
        }
    }
}

impl From<MediaKind> for media::MediaKind {
    #[inline]
    fn from(that: MediaKind) -> Self {
        match that {
            MediaKind::Audio => Self::Audio,
            MediaKind::Video => Self::Video,
        }
    }
}

impl From<media::MediaSourceKind> for MediaSourceKind {
    #[inline]
    fn from(that: media::MediaSourceKind) -> Self {
        match that {
            media::MediaSourceKind::Device => Self::Device,
            media::MediaSourceKind::Display => Self::Display,
        }
    }
}

impl From<MediaSourceKind> for media::MediaSourceKind {
    #[inline]
    fn from(that: MediaSourceKind) -> Self {
        match that {
            MediaSourceKind::Device => Self::Device,
            MediaSourceKind::Display => Self::Display,
        }
    }
}

impl From<media::FacingMode> for FacingMode {
    #[inline]
    fn from(that: media::FacingMode) -> Self {
        match that {
            media::FacingMode::User => Self::User,
            media::FacingMode::Environment => Self::Environment,
            media::FacingMode::Left => Self::Left,
            media::FacingMode::Right => Self::Right,
        }
    }
}

impl From<FacingMode> for media::FacingMode {
    #[inline]
    fn from(val: FacingMode) -> Self {
        match val {
            FacingMode::User => Self::User,
            FacingMode::Environment => Self::Environment,
            FacingMode::Left => Self::Left,
            FacingMode::Right => Self::Right,
        }
    }
}

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

/// Media source type.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, Display, Eq, PartialEq)]
pub enum MediaSourceKind {
    /// Media is sourced from some media device (webcam or microphone).
    Device,

    /// Media is obtained via screen capturing.
    Display,
}

/// Describes directions that a camera can face, as seen from a user's
/// perspective. Representation of a [VideoFacingModeEnum][1].
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-videofacingmodeenum
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, Display, Eq, PartialEq)]
pub enum FacingMode {
    /// Facing towards a user (a self-view camera).
    User,

    /// Facing away from a user (viewing the environment).
    Environment,

    /// Facing to the left of a user.
    Left,

    /// Facing to the right of a user.
    Right,
}

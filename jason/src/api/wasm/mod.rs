//! External Jason API for `wasm32-unknown-unknown` target, designed to be used
//! in web environment with Javascript.

#![allow(clippy::new_without_default)]

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

    /// Media is obtained with screen-capture.
    Display,
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

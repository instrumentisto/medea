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

pub use self::jason_error::JasonError;

/// [MediaStreamTrack.kind][1] representation.
///
/// [1]: https://w3.org/TR/mediacapture-streams#dom-mediastreamtrack-kind
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
/// [1]: https://w3.org/TR/mediacapture-streams#dom-videofacingmodeenum
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

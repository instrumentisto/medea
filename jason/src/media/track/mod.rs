//! [MediaStreamTrack][1] related objects.
//!
//! [1]: https://developer.mozilla.org/en-US/docs/Web/API/MediaStreamTrack

pub mod local;
pub mod remote;

use medea_client_api_proto::MediaSourceKind;
use wasm_bindgen::prelude::*;

/// Media source type.
#[wasm_bindgen(js_name = MediaSourceKind)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JsMediaSourceKind {
    /// Media is sourced from some media device (webcam or microphone).
    Device,

    /// Media is obtained with screen-capture.
    Display,
}

impl From<JsMediaSourceKind> for MediaSourceKind {
    fn from(val: JsMediaSourceKind) -> Self {
        match val {
            JsMediaSourceKind::Device => Self::Device,
            JsMediaSourceKind::Display => Self::Display,
        }
    }
}

impl From<MediaSourceKind> for JsMediaSourceKind {
    fn from(val: MediaSourceKind) -> Self {
        match val {
            MediaSourceKind::Device => Self::Device,
            MediaSourceKind::Display => Self::Display,
        }
    }
}

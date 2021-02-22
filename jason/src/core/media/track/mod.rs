//! [MediaStreamTrack][1] related objects.
//!
//! [1]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack

pub mod local;
pub mod remote;

use medea_client_api_proto as proto;

/// Media source type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaSourceKind {
    /// Media is sourced from some media device (webcam or microphone).
    Device,

    /// Media is obtained with screen-capture.
    Display,
}

impl From<MediaSourceKind> for proto::MediaSourceKind {
    #[inline]
    fn from(val: MediaSourceKind) -> Self {
        match val {
            MediaSourceKind::Device => Self::Device,
            MediaSourceKind::Display => Self::Display,
        }
    }
}

impl From<proto::MediaSourceKind> for MediaSourceKind {
    #[inline]
    fn from(val: proto::MediaSourceKind) -> Self {
        match val {
            proto::MediaSourceKind::Device => Self::Device,
            proto::MediaSourceKind::Display => Self::Display,
        }
    }
}

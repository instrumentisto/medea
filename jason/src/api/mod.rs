//! External Jason API.

mod wasm;

#[doc(inline)]
pub use wasm::{
    connection_handle::ConnectionHandle,
    constraints_update_exception::ConstraintsUpdateException,
    input_device_info::InputDeviceInfo,
    jason::Jason,
    jason_error::JasonError,
    local_media_track::LocalMediaTrack,
    media_manager_handle::MediaManagerHandle,
    media_stream_settings::{
        AudioTrackConstraints, DeviceVideoTrackConstraints,
        DisplayVideoTrackConstraints, MediaStreamSettings,
    },
    reconnect_handle::ReconnectHandle,
    remote_media_track::RemoteMediaTrack,
    room_close_reason::RoomCloseReason,
    room_handle::RoomHandle,
    FacingMode, MediaKind, MediaSourceKind,
};

use crate::media;

impl From<media::MediaKind> for MediaKind {
    fn from(that: media::MediaKind) -> Self {
        match that {
            media::MediaKind::Audio => Self::Audio,
            media::MediaKind::Video => Self::Video,
        }
    }
}

impl From<MediaKind> for media::MediaKind {
    fn from(that: MediaKind) -> Self {
        match that {
            MediaKind::Audio => Self::Audio,
            MediaKind::Video => Self::Video,
        }
    }
}

impl From<media::MediaSourceKind> for MediaSourceKind {
    fn from(that: media::MediaSourceKind) -> Self {
        match that {
            media::MediaSourceKind::Device => Self::Device,
            media::MediaSourceKind::Display => Self::Display,
        }
    }
}

impl From<MediaSourceKind> for media::MediaSourceKind {
    fn from(that: MediaSourceKind) -> Self {
        match that {
            MediaSourceKind::Device => Self::Device,
            MediaSourceKind::Display => Self::Display,
        }
    }
}

impl From<media::FacingMode> for FacingMode {
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
    fn from(val: FacingMode) -> Self {
        match val {
            FacingMode::User => Self::User,
            FacingMode::Environment => Self::Environment,
            FacingMode::Left => Self::Left,
            FacingMode::Right => Self::Right,
        }
    }
}

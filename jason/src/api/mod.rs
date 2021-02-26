//! External Jason API.

// #[cfg(all(
//     target_arch = "wasm32",
//     target_vendor = "unknown",
//     target_os = "unknown"
// ))]
mod wasm;

// #[cfg(all(
//     target_arch = "wasm32",
//     target_vendor = "unknown",
//     target_os = "unknown"
// ))]
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

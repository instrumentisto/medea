//! External [`Jason`] API.

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub mod wasm;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub use self::wasm::{
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

#[cfg(all(target_os = "android"))]
pub mod dart_ffi;

#[cfg(target_os = "android")]
pub use self::dart_ffi::{
    connection_handle::ConnectionHandle, input_device_info::InputDeviceInfo,
    jason_error::JasonError, local_media_track::LocalMediaTrack,
    media_manager::MediaManagerHandle,
    media_stream_settings::MediaStreamSettings,
    reconnect_handle::ReconnectHandle, remote_media_track::RemoteMediaTrack,
    room_close_reason::RoomCloseReason, room_handle::RoomHandle,
};

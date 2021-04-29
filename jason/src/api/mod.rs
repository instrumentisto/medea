//! External [`Jason`] API.

cfg_if::cfg_if! {
    if #[cfg(target_os = "android")] {
        pub mod dart;
        pub use self::dart::{
            connection_handle::ConnectionHandle, input_device_info::InputDeviceInfo,
            jason_error::JasonError, local_media_track::LocalMediaTrack,
            media_manager::MediaManagerHandle,
            media_stream_settings::MediaStreamSettings,
            reconnect_handle::ReconnectHandle, remote_media_track::RemoteMediaTrack,
            room_close_reason::RoomCloseReason, room_handle::RoomHandle,
        };
    } else {
        pub mod wasm;
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
    }
}

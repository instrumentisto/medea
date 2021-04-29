//! External [`Jason`] API that exposes functions that can be called via FFI
//! and designed to be integrated into Flutter plugin.
//!
//! [`Jason`]: crate::api::Jason

// TODO: Improve documentation in this module.
#![allow(clippy::missing_safety_doc, clippy::missing_panics_doc, missing_docs)]

pub mod audio_track_constraints;
pub mod connection_handle;
pub mod device_video_track_constraints;
pub mod display_video_track_constraints;
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
mod unimplemented;
pub mod utils;

pub use self::{
    audio_track_constraints::AudioTrackConstraints,
    connection_handle::ConnectionHandle,
    device_video_track_constraints::DeviceVideoTrackConstraints,
    display_video_track_constraints::DisplayVideoTrackConstraints,
    input_device_info::InputDeviceInfo, jason::Jason, jason_error::JasonError,
    local_media_track::LocalMediaTrack,
    media_manager_handle::MediaManagerHandle,
    media_stream_settings::MediaStreamSettings,
    reconnect_handle::ReconnectHandle, remote_media_track::RemoteMediaTrack,
    room_close_reason::RoomCloseReason, room_handle::RoomHandle,
};

/// Rust structure that has wrapper class in Dart. Such structures are passed
/// through FFI boundaries as thin pointers.
pub trait ForeignClass {
    /// Consumes `Self` returning a wrapped raw pointer via [`Box::into_raw`].
    fn into_ptr(self) -> *const Self
    where
        Self: Sized,
    {
        Box::into_raw(Box::new(self))
    }

    /// Constructs `Self` from a raw pointer via [`Box::from_raw`].
    unsafe fn from_ptr(this: *mut Self) -> Self
    where
        Self: Sized,
    {
        *Box::from_raw(this)
    }
}

//! External [`Jason`] API exposing functions that can be called via FFI and
//! designed to be integrated into a [Flutter] plugin.
//!
//! [`Jason`]: crate::api::Jason
//! [Flutter]: https://flutter.dev

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

/// Rust structure having wrapper class in Dart.
///
/// Intended to be passed through FFI boundaries as thin pointers.
pub trait ForeignClass: Sized {
    /// Consumes itself returning a wrapped raw pointer obtained via
    /// [`Box::into_raw()`].
    #[inline]
    #[must_use]
    fn into_ptr(self) -> *const Self {
        Box::into_raw(Box::new(self))
    }

    /// Constructs a [`ForeignClass`] from the given raw pointer via
    /// [`Box::from_raw()`].
    ///
    /// # Safety
    ///
    /// Same as for [`Box::from_raw()`].
    #[inline]
    #[must_use]
    unsafe fn from_ptr(this: *mut Self) -> Self {
        *Box::from_raw(this)
    }
}

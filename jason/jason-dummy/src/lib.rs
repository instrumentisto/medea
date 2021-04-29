#![allow(
    clippy::module_name_repetitions,
    clippy::unused_self,
    clippy::needless_pass_by_value,
    clippy::missing_safety_doc,
    clippy::must_use_candidate,
    clippy::missing_panics_doc,
    clippy::new_without_default
)]

use std::ptr::NonNull;

pub mod audio_track_constraints;
pub mod connection_handle;
pub mod device_video_track_constraints;
pub mod display_video_track_constraints;
pub mod input_device_info;
pub mod jason;
pub mod local_media_track;
pub mod media_manager;
pub mod media_stream_settings;
pub mod reconnect_handle;
pub mod remote_media_track;
pub mod room_close_reason;
pub mod room_handle;
mod unimplemented;
pub mod utils;

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
    unsafe fn from_ptr(this: NonNull<Self>) -> Self
    where
        Self: Sized,
    {
        *Box::from_raw(this.as_ptr())
    }
}

pub enum MediaKind {
    Audio = 0,
    Video = 1,
}

pub enum MediaSourceKind {
    Device = 0,
    Display = 1,
}

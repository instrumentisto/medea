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
    input_device_info::InputDeviceInfo,
    jason::Jason,
    jason_error::{DartError, JasonError},
    local_media_track::LocalMediaTrack,
    media_manager_handle::MediaManagerHandle,
    media_stream_settings::MediaStreamSettings,
    reconnect_handle::ReconnectHandle,
    remote_media_track::RemoteMediaTrack,
    room_close_reason::RoomCloseReason,
    room_handle::RoomHandle,
};

use std::{ffi::c_void, os::raw::c_char, ptr::NonNull};

use libc::c_char;

use crate::api::dart::utils::PtrArray;

/// Rust structure having wrapper class in Dart.
///
/// Intended to be passed through FFI boundaries as thin pointers.
pub trait ForeignClass: Sized {
    /// Consumes itself returning a wrapped raw pointer obtained via
    /// [`Box::into_raw()`].
    #[inline]
    #[must_use]
    fn into_ptr(self) -> NonNull<Self> {
        NonNull::from(Box::leak(Box::new(self)))
    }

    /// Constructs a [`ForeignClass`] from the given raw pointer via
    /// [`Box::from_raw()`].
    ///
    /// # Safety
    ///
    /// Same as for [`Box::from_raw()`].
    #[inline]
    #[must_use]
    unsafe fn from_ptr(this: NonNull<Self>) -> Self {
        *Box::from_raw(this.as_ptr())
    }
}

/// Value that can be transferred to Dart.
#[repr(u8)]
pub enum DartValue {
    Void,
    Ptr(NonNull<c_void>),
    String(NonNull<c_char>),
    Int(i64),
}

impl From<()> for DartValue {
    fn from(_: ()) -> Self {
        Self::Void
    }
}

impl<T: ForeignClass> From<T> for DartValue {
    fn from(val: T) -> Self {
        Self::Ptr(val.into_ptr().cast())
    }
}

impl<T> From<PtrArray<T>> for DartValue {
    fn from(val: PtrArray<T>) -> Self {
        DartValue::Ptr(NonNull::from(Box::leak(Box::new(val))).cast())
    }
}

impl From<NonNull<c_char>> for DartValue {
    fn from(c_str: NonNull<c_char>) -> Self {
        DartValue::String(c_str)
    }
}

impl From<NonNull<c_char>> for DartValue {
    fn from(from: NonNull<c_char>) -> Self {
        Self::String(from)
    }
}

/// Implements [`From`] for [`DartValue`] for types that can by casted to `i64`.
/// Should be called for all integer types that fit into `2^63`.
macro_rules! impl_from_num_for_dart_value {
    ($arg:ty) => {
        impl From<$arg> for DartValue {
            fn from(val: $arg) -> Self {
                DartValue::Int(i64::from(val))
            }
        }
    };
}

impl_from_num_for_dart_value!(i8);
impl_from_num_for_dart_value!(i16);
impl_from_num_for_dart_value!(i32);
impl_from_num_for_dart_value!(i64);
impl_from_num_for_dart_value!(u8);
impl_from_num_for_dart_value!(u16);
impl_from_num_for_dart_value!(u32);
impl_from_num_for_dart_value!(bool);

#![allow(
    clippy::module_name_repetitions,
    clippy::unused_self,
    clippy::needless_pass_by_value,
    clippy::missing_safety_doc,
    clippy::must_use_candidate,
    clippy::missing_panics_doc,
    clippy::new_without_default
)]

use std::ffi::c_void;

use crate::utils::PtrArray;

pub mod audio_track_constraints;
pub mod connection_handle;
pub mod device_video_track_constraints;
pub mod display_video_track_constraints;
pub mod input_device_info;
pub mod jason;
pub mod local_media_track;
pub mod media_manager_handle;
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
    unsafe fn from_ptr(this: *mut Self) -> Self
    where
        Self: Sized,
    {
        *Box::from_raw(this)
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

impl From<i32> for MediaSourceKind {
    fn from(from: i32) -> Self {
        match from {
            0 => Self::Device,
            1 => Self::Display,
            _ => unreachable!(),
        }
    }
}

/// Value that can be transferred to Dart.
pub enum DartValue {
    Void,
    Ptr(*const c_void),
    String(*const libc::c_char),
    PtrArray(PtrArray),
    Int(i64),
}

impl<T: ForeignClass> From<T> for DartValue {
    fn from(val: T) -> Self {
        Self::Ptr(val.into_ptr().cast())
    }
}

impl From<()> for DartValue {
    fn from(_: ()) -> Self {
        Self::Void
    }
}

impl<T> From<PtrArray<T>> for DartValue {
    fn from(val: PtrArray<T>) -> Self {
        DartValue::PtrArray(val.erase_type())
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

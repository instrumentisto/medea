use std::any::Any;

use dart_sys::Dart_Handle;

use crate::utils::dart::from_dart_string;

#[repr(C)]
pub struct DartOption {
    is_some: i8,
    val: Dart_Handle,
}

impl From<DartOption> for Option<Dart_Handle> {
    fn from(from: DartOption) -> Self {
        if from.is_some == 1 {
            unsafe { Some(from.val) }
        } else {
            None
        }
    }
}

#[repr(C)]
pub struct DartStringOption {
    is_some: i8,
    val: *const libc::c_char,
}

impl From<DartStringOption> for Option<String> {
    fn from(from: DartStringOption) -> Self {
        if from.is_some == 1 {
            unsafe { Some(from_dart_string(from.val)) }
        } else {
            None
        }
    }
}

#[repr(C)]
pub struct DartIntOption {
    is_some: i8,
    val: i32,
}

impl From<DartIntOption> for Option<i32> {
    fn from(from: DartIntOption) -> Self {
        if from.is_some == 1 {
            Some(from.val)
        } else {
            None
        }
    }
}

#[repr(C)]
pub struct DartUIntOption {
    is_some: i8,
    val: u32,
}

impl From<DartUIntOption> for Option<u32> {
    fn from(from: DartUIntOption) -> Self {
        if from.is_some == 1 {
            Some(from.val)
        } else {
            None
        }
    }
}
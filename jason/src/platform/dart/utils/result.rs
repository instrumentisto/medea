use std::any::Any;

use crate::{
    platform::dart::error::Error,
    utils::dart::{from_dart_string, into_dart_string},
};
use dart_sys::Dart_Handle;
use crate::platform::dart::utils::handle::DartHandle;

#[repr(C)]
pub struct DartResult {
    pub is_ok: i8,
    pub ok: *const dyn Any,
    pub err_message: *const libc::c_char,
    pub err_name: *const libc::c_char,
    pub cause: Dart_Handle,
}

impl<T: 'static> From<DartResult> for Result<&T, Error> {
    fn from(from: DartResult) -> Self {
        if from.is_ok == 1 {
            Ok(unsafe { from.ok.as_ref().unwrap().downcast_ref().unwrap() })
        } else {
            let message;
            let name;
            unsafe {
                message = from_dart_string(from.err_message);
                name = from_dart_string(from.err_name);
            }
            Err(Error { name, message, sys_cause: Some(DartHandle::new(from.cause)) })
        }
    
    }
}

#[repr(C)]
pub struct VoidDartResult {
    pub is_ok: i8,
    pub err_name: *const libc::c_char,
    pub err_message: *const libc::c_char,
    pub cause: Dart_Handle,
}

impl From<VoidDartResult> for Result<(), Error> {
    fn from(from: VoidDartResult) -> Self {
        if from.is_ok == 1 {
            Ok(())
        } else {
            let message;
            let name;
            unsafe {
                message = from_dart_string(from.err_message);
                name = from_dart_string(from.err_name);
            }
            Err(Error { name, message, sys_cause: Some(DartHandle::new(from.cause)) })
        }
    }
}

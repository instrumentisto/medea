use std::any::Any;

use dart_sys::Dart_Handle;

use crate::{
    api::dart::utils::c_str_into_string,
    platform::dart::{error::Error, utils::handle::DartHandle},
};

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
                message = c_str_into_string(from.err_message);
                name = c_str_into_string(from.err_name);
            }
            Err(Error {
                name,
                message,
                sys_cause: Some(DartHandle::new(from.cause)),
            })
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
                message = c_str_into_string(from.err_message);
                name = c_str_into_string(from.err_name);
            }
            Err(Error {
                name,
                message,
                sys_cause: Some(DartHandle::new(from.cause)),
            })
        }
    }
}

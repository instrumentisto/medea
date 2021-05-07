use std::ptr;

use libc::{c_char, c_void};

use crate::api::dart::{
    jason_error::DartError, utils::PtrArray, DartValue, JasonError,
};

/// Dart structure which represents [`Result`] for the Dart error.
#[repr(C)]
pub struct DartResult {
    /// Pointer to the [`ForeignClass`] for the [`Ok`].
    pub ptr_ok: *const c_void,

    /// Pointer to the [`PtrArray`] for the [`Ok`].
    pub arr_ok: PtrArray,

    /// Pointer to the [`String`] for the [`Ok`].
    pub str_ok: *const c_char,

    /// [`i64`] for the [`Ok`].
    pub int_ok: i64,

    /// Type of the [`Ok`] variant.
    pub ok_type: i32,

    /// [`DartError`] for the [`Err`] variant.
    pub error: DartError,

    /// Boolean which indicates that this [`DartResult`] is ok.
    pub is_ok: i32,
}

impl DartResult {
    /// Returns [`DartResult`] for the [`Ok`] variant.
    fn ok(val: DartValue) -> Self {
        let mut ptr_ok = ptr::null();
        let mut arr_ok = PtrArray::null();
        let mut str_ok = ptr::null();
        let mut int_ok = 0;
        let ok_type = val.id();
        match val {
            DartValue::Ptr(ptr) => {
                ptr_ok = ptr;
            }
            DartValue::Int(i) => {
                int_ok = i;
            }
            DartValue::String(s) => {
                str_ok = s;
            }
            DartValue::PtrArray(a) => {
                arr_ok = a;
            }
            DartValue::Void => (),
        };

        Self {
            ptr_ok,
            arr_ok,
            str_ok,
            int_ok,
            ok_type,
            error: DartError::null(),
            is_ok: 1,
        }
    }

    /// Returns [`DartResult`] for the [`Err`] variant.
    fn err(err: JasonError) -> Self {
        Self {
            ptr_ok: ptr::null(),
            arr_ok: PtrArray::null(),
            str_ok: ptr::null(),
            int_ok: 0,
            ok_type: -1,
            error: err.into(),
            is_ok: 0,
        }
    }
}

impl<T> From<Result<T, JasonError>> for DartResult
where
    T: Into<DartValue>,
{
    fn from(res: Result<T, JasonError>) -> Self {
        match res {
            Ok(val) => Self::ok(val.into()),
            Err(e) => Self::err(e),
        }
    }
}

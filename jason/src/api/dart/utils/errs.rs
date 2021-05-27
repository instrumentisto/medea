//! Facility for creating Dart exceptions from Rust.

use std::{borrow::Cow, ptr};

use dart_sys::Dart_Handle;
use derive_more::Into;
use libc::c_char;

use crate::api::dart::{utils::string_into_c_str, DartValue};

/// Pointer to an extern function that returns a new Dart [`ArgumentError`] with
/// the provided invalid argument, its name and error message describing the
/// problem.
///
/// [`ArgumentError`]: https://api.dart.dev/dart-core/ArgumentError-class.html
type NewArgumentErrorCaller = extern "C" fn(
    value: DartValue,
    arg_name: ptr::NonNull<c_char>,
    msg: ptr::NonNull<c_char>,
) -> Dart_Handle;

/// Stores pointer to the [`NewArgumentErrorCaller`] extern function.
///
/// Must be initialized by Dart during FFI initialization phase.
static mut NEW_ARGUMENT_ERROR_CALLER: Option<NewArgumentErrorCaller> = None;

/// An error that can be returned from Rust to Dart.
#[derive(Into)]
#[repr(transparent)]
pub struct DartError(ptr::NonNull<Dart_Handle>);

impl DartError {
    /// Creates a new [`DartError`] from the provided [`Dart_Handle`].
    #[inline]
    fn new(handle: Dart_Handle) -> DartError {
        DartError(ptr::NonNull::from(Box::leak(Box::new(handle))))
    }
}

/// Registers the provided [`NewArgumentErrorCaller`] as
/// [`NEW_ARGUMENT_ERROR_CALLER`].
///
/// # Safety
///
/// Must ONLY be called by Dart during FFI initialization.
#[no_mangle]
pub unsafe extern "C" fn register_new_argument_error_caller(
    f: NewArgumentErrorCaller,
) {
    NEW_ARGUMENT_ERROR_CALLER = Some(f);
}

/// Error that Rust returns when a function is passed an unacceptable argument.
///
/// It can be converted into [`DartError`] and passed to Dart.
pub struct ArgumentError<T> {
    /// The invalid value.
    val: T,
    /// Name of the invalid argument.
    name: &'static str,
    /// Message describing the problem.
    message: Cow<'static, str>,
}

impl<T> ArgumentError<T> {
    /// Creates new [`ArgumentError`] from provided invalid argument, its name
    /// and error message.
    pub fn new<V>(val: T, name: &'static str, message: V) -> Self
    where
        V: Into<Cow<'static, str>>,
    {
        Self {
            val,
            name,
            message: message.into(),
        }
    }
}

impl<T: Into<DartValue>> From<ArgumentError<T>> for DartError {
    #[inline]
    fn from(err: ArgumentError<T>) -> Self {
        unsafe {
            DartError::new(NEW_ARGUMENT_ERROR_CALLER.unwrap()(
                err.val.into(),
                string_into_c_str(err.name.to_owned()),
                string_into_c_str(err.message.into_owned()),
            ))
        }
    }
}

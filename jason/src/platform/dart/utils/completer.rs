//! Proxy for a Dart's [Completer].
//!
//! Rust doesn't have direct access to the Dart's [Completer], but holds a
//! [`Dart_PersistentHandle`] to the [Completer] instance. All manipulations
//! happen on the Dart side.
//!
//! Dart side must register static functions that Rust will call to manipulate
//! the [Completer]. This module exports function for registering those Dart
//! functions: [`register_new_completer_caller`],
//! [`register_completer_complete_caller`],
//! [`register_completer_complete_error_caller`],
//! [`register_completer_future_caller`],
//!
//! These functions MUST be registered by Dart during FFI initialization phase:
//! after Dart DL API is initialized and before any other exported Rust function
//! is called.
//!
//! [Completer]: https://api.dart.dev/dart-async/Completer-class.html

use std::{ffi::c_void, marker::PhantomData, ptr::NonNull};

use dart_sys::{Dart_Handle, Dart_PersistentHandle};

use crate::api::DartValue;

use super::dart_api::{
    Dart_HandleFromPersistent_DL_Trampolined,
    Dart_NewPersistentHandle_DL_Trampolined,
};

/// Pointer to an extern function that returns [`Dart_Handle`] to a new Dart
/// [Completer].
///
/// [Completer]: https://api.dart.dev/dart-async/Completer-class.html
type CompleterNewCaller = extern "C" fn() -> Dart_Handle;

/// Pointer to an extern function that invokes [complete] function with the
/// provided [`DartValue`] on the provided [`Dart_Handle`] which points to the
/// Dart [Completer] object.
///
/// [complete]: https://api.dart.dev/dart-async/Completer/complete.html
/// [Completer]: https://api.dart.dev/dart-async/Completer-class.html
type CompleterCompleteCaller = extern "C" fn(Dart_Handle, DartValue);

/// Pointer to an extern function that invokes the [completeError][1] method
/// with the provided Rust pointer on the provided [`Dart_Handle`] which points
/// to the Dart [Completer] object.
///
/// [1]: https://api.dart.dev/dart-async/Completer/completeError.html
/// [Completer]: https://api.dart.dev/dart-async/Completer-class.html
type CompleterCompleteErrorCaller = extern "C" fn(Dart_Handle, NonNull<c_void>);

/// Pointer to an extern function that calls [future] getter on the provided
/// [`Dart_Handle`] which points to the Dart [Completer] object.
///
/// This function will return [`Dart_Handle`] to the Dart [Future] which can be
/// returned to the Dart side.
///
/// [future]: https://api.dart.dev/dart-async/Completer/future.html
/// [Completer]: https://api.dart.dev/dart-async/Completer-class.html
/// [Future]: https://api.dart.dev/dart-async/Future-class.html
type CompleterFutureCaller = extern "C" fn(Dart_Handle) -> Dart_Handle;

/// Stores pointer to the [`CompleterNewCaller`] extern function.
///
/// Should be initialized by Dart during FFI initialization phase.
static mut COMPLETER_NEW_CALLER: Option<CompleterNewCaller> = None;

/// Stores pointer to the [`CompleterCompletePtrCaller`] extern function.
///
/// Should be initialized by Dart during FFI initialization phase.
static mut COMPLETER_COMPLETE_CALLER: Option<CompleterCompleteCaller> = None;

/// Stores pointer to the [`CompleterCompleteErrorCaller`] extern function.
///
/// Should be initialized by Dart during FFI initialization phase.
static mut COMPLETER_COMPLETE_ERROR_CALLER: Option<
    CompleterCompleteErrorCaller,
> = None;

/// Stores pointer to [`CompleterFutureCaller`] extern function.
///
/// Should be initialized by Dart during FFI initialization phase.
static mut COMPLETER_FUTURE_CALLER: Option<CompleterFutureCaller> = None;

/// Registers the provided [`CompleterNewCaller`] as [`COMPLETER_NEW_CALLER`].
///
/// # Safety
///
/// Must ONLY be called by Dart during FFI initialization.
#[no_mangle]
pub unsafe extern "C" fn register_new_completer_caller(f: CompleterNewCaller) {
    COMPLETER_NEW_CALLER = Some(f);
}

/// Registers the provided [`CompleterCompleteCaller`] as
/// [`COMPLETER_COMPLETE_CALLER`].
///
/// # Safety
///
/// Must ONLY be called by Dart during FFI initialization.
#[no_mangle]
pub unsafe extern "C" fn register_completer_complete_caller(
    f: CompleterCompleteCaller,
) {
    COMPLETER_COMPLETE_CALLER = Some(f);
}

/// Registers the provided [`CompleterCompleteErrorCaller`] as
/// [`COMPLETER_COMPLETE_ERROR_CALLER`].
///
/// # Safety
///
/// Must ONLY be called by Dart during FFI initialization.
#[no_mangle]
pub unsafe extern "C" fn register_completer_complete_error_caller(
    f: CompleterCompleteErrorCaller,
) {
    COMPLETER_COMPLETE_ERROR_CALLER = Some(f);
}

/// Registers the provided [`CompleterFutureCaller`] as
/// [`COMPLETER_FUTURE_CALLER`].
///
/// # Safety
///
/// Must ONLY be called by Dart during FFI initialization.
#[no_mangle]
pub unsafe extern "C" fn register_completer_future_caller(
    f: CompleterFutureCaller,
) {
    COMPLETER_FUTURE_CALLER = Some(f);
}

/// Dart [Future] which can be resolved from Rust.
///
/// [Future]: https://api.dart.dev/dart-async/Future-class.html
pub struct Completer<T, E> {
    /// [`Dart_PersistentHandle`] to the Dart `Completer` that backs this
    /// [`Completer`].
    handle: Dart_PersistentHandle,

    /// Type with which [Future] can be successfully resolved.
    ///
    /// [Future]: https://api.dart.dev/dart-async/Future-class.html
    _success_kind: PhantomData<*const T>,

    /// Type with which [Future] can be resolved on error.
    ///
    /// [Future]: https://api.dart.dev/dart-async/Future-class.html
    _error_kind: PhantomData<*const E>,
}

impl<T, E> Completer<T, E> {
    /// Creates a new [`Dart_PersistentHandle`] for the Dart [`Completer`].
    ///
    /// Persists the created [`Dart_Handle`] so it won't be moved by the Dart VM
    /// GC.
    #[must_use]
    pub fn new() -> Self {
        let handle = unsafe {
            let completer = COMPLETER_NEW_CALLER.unwrap()();
            Dart_NewPersistentHandle_DL_Trampolined(completer)
        };
        Self {
            handle,
            _success_kind: PhantomData::default(),
            _error_kind: PhantomData::default(),
        }
    }

    /// Returns [`Dart_Handle`] to the Dart [Future] controlled by this
    /// [`Completer`].
    ///
    /// [Future]: https://api.dart.dev/dart-async/Future-class.html
    #[must_use]
    pub fn future(&self) -> Dart_Handle {
        unsafe {
            let handle = Dart_HandleFromPersistent_DL_Trampolined(self.handle);
            COMPLETER_FUTURE_CALLER.unwrap()(handle)
        }
    }
}

impl<T, E> Default for Completer<T, E> {
    fn default() -> Self {
        Completer::new()
    }
}

impl<T: Into<DartValue>, E> Completer<T, E> {
    /// Successfully completes underlying Dart [Future] with a provided
    /// argument.
    ///
    /// [Future]: https://api.dart.dev/dart-async/Future-class.html
    pub fn complete(&self, arg: T) {
        unsafe {
            let handle = Dart_HandleFromPersistent_DL_Trampolined(self.handle);
            COMPLETER_COMPLETE_CALLER.unwrap()(handle, arg.into());
        }
    }
}

impl<T, E: Into<DartValue>> Completer<T, E> {
    /// Completes underlying Dart [Future] with error provided as the argument.
    ///
    /// [Future]: https://api.dart.dev/dart-async/Future-class.html
    pub fn complete_error(&self, _: E) {
        todo!("COMPLETER_COMPLETE_ERROR_CALLER call")
    }
}

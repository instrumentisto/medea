//! Functionality for running Rust Futures on Dart side.

use std::{any::Any, marker::PhantomData};

use dart_sys::{Dart_Handle, Dart_PersistentHandle};

use crate::utils::PtrArray;

use super::dart_api::{
    Dart_HandleFromPersistent_DL_Trampolined,
    Dart_NewPersistentHandle_DL_Trampolined,
};

/// Pointer to an extern function that returns [`Dart_Handle`] to the new Dart
/// `Completer`.
type NewCompleterCaller = extern "C" fn() -> Dart_Handle;

/// Pointer to an extern function that invokes `complete` function with provided
/// Rust pointer on the provided [`Dart_Handle`] which should point to the Dart
/// `Completer` object.
type CompleterCompleteCaller = extern "C" fn(c: Dart_Handle, var: *mut dyn Any);

/// Pointer to an extern function that invokes `completeError` function with
/// provided Rust pointer on the provided [`Dart_Handle`] which should point to
/// the Dart `Completer` object.
type CompleterCompleteErrorCaller =
    extern "C" fn(c: Dart_Handle, var: *mut dyn Any);

/// Pointer to an extern function that invokes `complete` function with provided
/// [`PtrArray`] on the provided [`Dart_Handle`] which should point to the Dart
/// `Completer` object.
type ArrayCompleterCompleteCaller = extern "C" fn(Dart_Handle, var: PtrArray);

/// Pointer to an extern function that invokes `future` function on the provided
/// [`Dart_Handle`] which should point to the Dart `Completer` object.
///
/// This function will return [`Dart_Handle`] to the Dart `Future` which can be
/// returned to the Dart side.
type CompleterFutureCaller = extern "C" fn(c: Dart_Handle) -> Dart_Handle;

/// Stores [`NewCompleter`] extern function.
///
/// Should be initialized by Dart during FFI initialization phase.
static mut NEW_COMPLETER_CALLER: Option<NewCompleterCaller> = None;

/// Stores [`CompleterCompleteCaller`] extern function.
///
/// Should be initialized by Dart during FFI initialization phase.
static mut COMPLETER_COMPLETE_CALLER: Option<CompleterCompleteCaller> = None;

/// Stores [`CompleterCompleteErrorCaller`] extern function.
///
/// Should be initialized by Dart during FFI initialization phase.
static mut COMPLETER_COMPLETE_ERROR_CALLER: Option<
    CompleterCompleteErrorCaller,
> = None;

/// Stores [`ArrayCompleterComplete`] extern function.
///
/// Should be initialized by Dart during FFI initialization phase.
static mut ARRAY_COMPLETER_COMPLETE_CALLER: Option<
    ArrayCompleterCompleteCaller,
> = None;

/// Stores [`CompleterFutureCaller`] extern function.
///
/// Should be initialized by Dart during FFI initialization phase.
static mut COMPLETER_FUTURE_CALLER: Option<CompleterFutureCaller> = None;

/// Registers the provided [`NewCompleterCaller`] as [`NEW_COMPLETER_CALLER`].
/// Must be called by dart during FFI initialization.
#[no_mangle]
pub unsafe extern "C" fn register_new_completer_caller(f: NewCompleterCaller) {
    NEW_COMPLETER_CALLER = Some(f);
}

/// Registers the provided [`CompleterCompleteCaller`] as
/// [`COMPLETER_COMPLETE_CALLER`]. Must be called by dart during FFI
/// initialization.
#[allow(improper_ctypes_definitions)]
#[no_mangle]
pub unsafe extern "C" fn register_completer_complete_caller(
    f: CompleterCompleteCaller,
) {
    COMPLETER_COMPLETE_CALLER = Some(f);
}

/// Registers the provided [`CompleterCompleteErrorCaller`] as
/// [`COMPLETER_COMPLETE_ERROR_CALLER`]. Must be called by dart during FFI
/// initialization.
#[allow(improper_ctypes_definitions)]
#[no_mangle]
pub unsafe extern "C" fn register_completer_complete_error_caller(
    f: CompleterCompleteErrorCaller,
) {
    COMPLETER_COMPLETE_ERROR_CALLER = Some(f);
}

/// Registers the provided [`ArrayCompleterCompleteCaller`] as
/// [`ARRAY_COMPLETER_COMPLETE_CALLER`]. Must be called by dart during FFI
/// initialization.
#[no_mangle]
pub unsafe extern "C" fn register_array_completer_complete_caller(
    f: ArrayCompleterCompleteCaller,
) {
    ARRAY_COMPLETER_COMPLETE_CALLER = Some(f);
}

/// Registers the provided [`CompleterFutureCaller`] as
/// [`COMPLETER_FUTURE_CALLER`]. Must be called by dart during FFI
/// initialization.
#[no_mangle]
pub unsafe extern "C" fn register_completer_future_caller(
    f: CompleterFutureCaller,
) {
    COMPLETER_FUTURE_CALLER = Some(f);
}

/// Dart `Future` which can be resolved from Rust.
pub struct Completer<O, E> {
    /// [`Dart_PersistentHandle`] to the Dart `Completer` which should be
    /// resolved.
    handle: Dart_PersistentHandle,

    /// Type with which `Future` can be successfully resolved.
    _success: PhantomData<O>,

    /// Type with which `Future` can be resolved on error.
    _error: PhantomData<E>,
}

impl<O: 'static, E: 'static> Completer<O, E> {
    /// Creates a new [`Dart_PersistentHandle`] for the Dart [`Completer`].
    ///
    /// Persists the created [`Dart_Handle`] so it won't be moved by the Dart VM
    /// GC.
    pub fn new() -> Self {
        let completer;
        let persist;
        unsafe {
            completer = NEW_COMPLETER_CALLER.unwrap()();
            persist = Dart_NewPersistentHandle_DL_Trampolined(completer);
        }
        Self {
            handle: persist,
            _success: PhantomData::default(),
            _error: PhantomData::default(),
        }
    }

    /// Successfully completes underlying Dart `Future` with a provided
    /// argument.
    pub fn complete(&self, arg: O) {
        unsafe {
            let handle = Dart_HandleFromPersistent_DL_Trampolined(self.handle);
            COMPLETER_COMPLETE_CALLER.unwrap()(
                handle,
                Box::into_raw(Box::new(arg) as Box<dyn Any>),
            );
        }
    }

    /// Completes underlying Dart `Future` with error provided as the argument.
    pub fn complete_error(&self, err: E) {
        unsafe {
            let handle = Dart_HandleFromPersistent_DL_Trampolined(self.handle);
            COMPLETER_COMPLETE_ERROR_CALLER.unwrap()(
                handle,
                Box::into_raw(Box::new(err) as Box<dyn Any>),
            );
        }
    }

    /// Returns [`Dart_Handle`] to the Dart `Future` controlled by this
    /// [`Completer`].
    pub fn future(&self) -> Dart_Handle {
        unsafe {
            let handle = Dart_HandleFromPersistent_DL_Trampolined(self.handle);
            COMPLETER_FUTURE_CALLER.unwrap()(handle)
        }
    }
}

impl<E: 'static> Completer<PtrArray, E> {
    /// Successfully completes underlying Dart `Future` with a provided
    /// [`PtrArray`].
    pub fn complete_with_array(&self, arg: PtrArray) {
        unsafe {
            let handle = Dart_HandleFromPersistent_DL_Trampolined(self.handle);
            ARRAY_COMPLETER_COMPLETE_CALLER.unwrap()(handle, arg);
        }
    }
}

unsafe impl<O, E> Send for Completer<O, E> {}

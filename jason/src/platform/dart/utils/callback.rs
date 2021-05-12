//! Functionality for calling Dart closures from Rust.
//!
//! Dart DL API doesn't allow calling Dart closures directly. So Dart registers
//! static functions that accept and invoke the provided Dart closures. This
//! module exports function for registering "caller" functions:
//! [`register_ptr_arg_fn_caller`], [`register_no_args_fn_caller`],
//! [`register_int_arg_fn_caller`].
//!
//! These "caller" functions MUST be registered by Dart during FFI
//! initialization phase: after Dart DL API is initialized and before any other
//! exported Rust function is called.

use std::{cell::RefCell, ffi::c_void, marker::PhantomData};

use dart_sys::{Dart_Handle, Dart_PersistentHandle};

use crate::api::DartValue;

use super::dart_api::{
    Dart_DeletePersistentHandle_DL_Trampolined,
    Dart_HandleFromPersistent_DL_Trampolined,
    Dart_NewPersistentHandle_DL_Trampolined,
};

/// Pointer to an extern function that accepts a [`Dart_Handle`] and a
/// `*const c_void` pointer.
type PointerArgFnCaller = extern "C" fn(Dart_Handle, *const c_void);

/// Pointer to an extern function that accepts a [`Dart_Handle`].
type UnitArgFnCaller = extern "C" fn(Dart_Handle);

/// Pointer to an extern function that accepts a [`Dart_Handle`] and a `i64`
/// number.
type IntArgFnCaller = extern "C" fn(Dart_Handle, i64);

/// Dart function used to invoke Dart closures that accept a `*const c_void`
/// argument.
static mut PTR_ARG_FN_CALLER: Option<PointerArgFnCaller> = None;

/// Dart function used to invoke other Dart closures without arguments.
static mut NO_ARGS_FN_CALLER: Option<UnitArgFnCaller> = None;

/// Dart function used to invoke other Dart closures that accept an `i64`
/// argument.
static mut INT_ARG_FN_CALLER: Option<IntArgFnCaller> = None;

/// Registers the provided [`PointerArgFnCaller`] as [`PTR_ARG_FN_CALLER`].
///
/// # Safety
///
/// Must ONLY be called by Dart during FFI initialization.
#[no_mangle]
pub unsafe extern "C" fn register_ptr_arg_fn_caller(
    caller: PointerArgFnCaller,
) {
    PTR_ARG_FN_CALLER = Some(caller);
}

/// Registers the provided [`UnitArgFnCaller`] as [`NO_ARGS_FN_CALLER`].
///
/// # Safety
///
/// Must ONLY be called by Dart during FFI initialization.
#[no_mangle]
pub unsafe extern "C" fn register_no_args_fn_caller(f: UnitArgFnCaller) {
    NO_ARGS_FN_CALLER = Some(f)
}

/// Registers the provided [`IntArgFnCaller`] as [`INT_ARG_FN_CALLER`].
///
/// # Safety
///
/// Must ONLY be called by Dart during FFI initialization.
#[no_mangle]
pub unsafe extern "C" fn register_int_arg_fn_caller(f: IntArgFnCaller) {
    INT_ARG_FN_CALLER = Some(f);
}

// TODO: Probably should be shared between `wasm` and `dart` platforms.
/// Wrapper for a single argument Dart callback function.
pub struct Callback<A>(RefCell<Option<Function<A>>>);

impl<A> Callback<A> {
    /// Sets the inner [`Function`].
    #[inline]
    pub fn set_func(&self, f: Function<A>) {
        self.0.borrow_mut().replace(f);
    }

    /// Indicates whether this [`Callback`]'s inner [`Function`] is set.
    #[inline]
    #[must_use]
    pub fn is_set(&self) -> bool {
        self.0.borrow().as_ref().is_some()
    }
}

impl Callback<()> {
    /// Invokes the underlying [`Function`] (if any) passing no arguments to it.
    #[inline]
    pub fn call0(&self) {
        if let Some(f) = self.0.borrow().as_ref() {
            f.call0()
        };
    }
}

impl<A> Default for Callback<A> {
    #[inline]
    fn default() -> Self {
        Self(RefCell::new(None))
    }
}

impl<A> Callback<A> {
    /// Invokes the underlying [`Function`] (if any) passing the single provided
    /// argument to it.
    #[inline]
    pub fn call1<T>(&self, arg: T) {
        unimplemented!()
    }
}

// TODO: Print exception if Dart closure throws.
/// Dart closure that can be called from Rust.
pub struct Function<T> {
    /// [`Dart_PersistentHandle`] to the Dart closure that should be called.
    dart_fn: Dart_PersistentHandle,

    /// Type of this closure argument.
    _arg: PhantomData<*const T>,
}

impl<T> Function<T> {
    /// Creates a new [`Function`] from the provided [`Dart_Handle`] to a Dart
    /// closure, and persists the provided [`Dart_Handle`] so it won't be moved
    /// by the Dart VM GC.
    #[inline]
    #[must_use]
    pub fn new(cb: Dart_Handle) -> Self {
        Self {
            dart_fn: unsafe { Dart_NewPersistentHandle_DL_Trampolined(cb) },
            _arg: PhantomData,
        }
    }
}

impl Function<()> {
    /// Calls the underlying Dart closure.
    #[inline]
    pub fn call0(&self) {
        self.call1(());
    }
}

impl<T: Into<DartValue>> Function<T> {
    /// Calls the underlying Dart closure with the provided argument.
    #[inline]
    pub fn call1(&self, arg: T) {
        unsafe {
            let fn_handle =
                Dart_HandleFromPersistent_DL_Trampolined(self.dart_fn);

            match arg.into() {
                DartValue::Ptr(ptr) => {
                    PTR_ARG_FN_CALLER.unwrap()(fn_handle, ptr);
                }
                DartValue::Int(int) => {
                    INT_ARG_FN_CALLER.unwrap()(fn_handle, int);
                }
                DartValue::Void => {
                    NO_ARGS_FN_CALLER.unwrap()(fn_handle);
                }
                DartValue::String(_) | DartValue::PtrArray(_) => {
                    todo!()
                }
            }
        }
    }
}

impl<T> Drop for Function<T> {
    /// Manually deallocates saved [`Dart_PersistentHandle`] so it won't leak.
    fn drop(&mut self) {
        unsafe { Dart_DeletePersistentHandle_DL_Trampolined(self.dart_fn) };
    }
}

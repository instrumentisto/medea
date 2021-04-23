//! Functionality for calling Dart closures from Rust.
//!
//! Dart DL API does not allow calling Dart closures directly. So Dart registers
//! static functions that accept and invoke provided Dart closures. This module
//! exports function for registering "caller" functions:
//! [`register_ptr_arg_fn_caller`], [`register_no_args_fn_caller`],
//! [`register_int_arg_fn_caller`].
//!
//! These "caller" functions MUST be registered by Dart during FFI
//! initialization phase: after Dart DL API initialization and before calling
//! any other exported Rust function.

use std::marker::PhantomData;

use dart_sys::{Dart_Handle, Dart_PersistentHandle};

use crate::ForeignClass;

use super::dart_api::{
    Dart_DeletePersistentHandle_DL_Trampolined,
    Dart_HandleFromPersistent_DL_Trampolined,
    Dart_NewPersistentHandle_DL_Trampolined,
};

/// Pointer to an extern function that accepts [`Dart_Handle`] and `*const ()`;
type PointerArgFnCaller = extern "C" fn(Dart_Handle, *const ());

/// Pointer to an extern function that accepts [`Dart_Handle`];
type UnitArgFnCaller = extern "C" fn(Dart_Handle);

/// Pointer to an extern function that accepts [`Dart_Handle`] and `i64`;
type IntArgFnCaller = extern "C" fn(Dart_Handle, i64);

/// Dart function used to invoke Dart closures that accept an `*const ()`
/// argument.
static mut PTR_ARG_FN_CALLER: Option<PointerArgFnCaller> = None;

/// Dart function used to invoke other Dart closures without arguments.
static mut NO_ARGS_FN_CALLER: Option<UnitArgFnCaller> = None;

/// Dart function used to invoke other Dart closures that accept an `i64`
/// argument.
static mut INT_ARG_FN_CALLER: Option<IntArgFnCaller> = None;

/// Registers the provided [`PointerArgFnCaller`] as [`PTR_ARG_FN_CALLER`]. Must
/// be called by dart during FFI initialization.
#[no_mangle]
pub unsafe extern "C" fn register_ptr_arg_fn_caller(
    caller: PointerArgFnCaller,
) {
    PTR_ARG_FN_CALLER = Some(caller);
}

/// Registers the provided [`UnitArgFnCaller`] as [`NO_ARGS_FN_CALLER`]. Must be
/// called by dart during FFI initialization.
#[no_mangle]
pub unsafe extern "C" fn register_no_args_fn_caller(f: UnitArgFnCaller) {
    NO_ARGS_FN_CALLER = Some(f)
}

/// Registers the provided [`IntArgFnCaller`] as [`INT_ARG_FN_CALLER`]. Must be
/// called by dart during FFI initialization.
#[no_mangle]
pub unsafe extern "C" fn register_int_arg_fn_caller(f: IntArgFnCaller) {
    INT_ARG_FN_CALLER = Some(f);
}

// TODO: Print exception if Dart closure throws.
/// Dart closure that can be called from Rust.
pub struct DartClosure<T> {
    /// [`Dart_PersistentHandle`] to the Dart closure that should be called.
    cb: Dart_PersistentHandle,
    _arg_kind: PhantomData<T>,
}

impl<T> DartClosure<T> {
    /// Creates new [`DartClosure`] from the provided [`Dart_Handle`] to the
    /// Dart closure. Persists the provided [`Dart_Handle`] so it wont be moved
    /// by the Dart VM GC.
    pub fn new(cb: Dart_Handle) -> Self {
        Self {
            cb: unsafe { Dart_NewPersistentHandle_DL_Trampolined(cb) },
            _arg_kind: PhantomData::default(),
        }
    }
}

impl DartClosure<()> {
    /// Calls underlying Dart closure.
    pub fn call0(&self) {
        unsafe {
            let fn_handle = Dart_HandleFromPersistent_DL_Trampolined(self.cb);
            NO_ARGS_FN_CALLER.unwrap()(fn_handle);
        }
    }
}

impl<T: ForeignClass> DartClosure<T> {
    /// Calls underlying Dart closure with provided [`ForeignClass`] argument.
    pub fn call1(&self, arg: T) {
        unsafe {
            let fn_handle = Dart_HandleFromPersistent_DL_Trampolined(self.cb);
            PTR_ARG_FN_CALLER.unwrap()(fn_handle, arg.into_ptr().cast::<()>());
        }
    }
}

/// Implements [`DartClosure::call`] casting argument to `i64`. Should be
/// called for all integer types that fit into 2^63.
macro_rules! impl_dart_closure_for_int {
    ($arg:ty) => {
        impl DartClosure<$arg> {
            /// Calls underlying Dart closure with provided argument.
            pub fn call1(&self, arg: $arg) {
                unsafe {
                    let fn_handle =
                        Dart_HandleFromPersistent_DL_Trampolined(self.cb);
                    INT_ARG_FN_CALLER.unwrap()(fn_handle, arg as i64);
                }
            }
        }
    };
}

impl_dart_closure_for_int!(i8);
impl_dart_closure_for_int!(i16);
impl_dart_closure_for_int!(i32);
impl_dart_closure_for_int!(i64);
impl_dart_closure_for_int!(isize);
impl_dart_closure_for_int!(u8);
impl_dart_closure_for_int!(u16);
impl_dart_closure_for_int!(u32);
impl_dart_closure_for_int!(bool);

impl<T> Drop for DartClosure<T> {
    /// Manually deallocate saved [`Dart_PersistentHandle`] so it wont leak.
    fn drop(&mut self) {
        unsafe { Dart_DeletePersistentHandle_DL_Trampolined(self.cb) };
    }
}

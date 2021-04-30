//! Functionality for calling [`Dart DL API`] from Rust.
//!
//! [`Dart DL API`]: https://tinyurl.com/32e7fudh

use dart_sys::{Dart_Handle, Dart_PersistentHandle};

#[link(name = "trampoline")]
extern "C" {
    /// Initializes Dynamically Linked Dart API usage. Accepts
    /// [`NativeApi.initializeApiDLData`][1] that must be retrieved in Dart
    /// code. Must be called before calling any other Dart API method.
    ///
    /// [1]: https://api.dart.dev/dart-ffi/NativeApi/initializeApiDLData.html
    pub fn Dart_InitializeApiDL(obj: *mut libc::c_void) -> libc::intptr_t;

    /// Allocates a [`Dart_PersistentHandle`] for provided [`Dart_Handle`].
    ///
    /// [`Dart_PersistentHandle`]s have the lifetime of the current isolate
    /// unless they are explicitly deallocated.
    pub fn Dart_NewPersistentHandle_DL_Trampolined(
        object: Dart_Handle,
    ) -> Dart_PersistentHandle;

    /// Allocates a [`Dart_Handle`] in the current scope from the given
    /// [`Dart_PersistentHandle`].
    ///
    /// This doesn't affect the provided [`Dart_PersistentHandle`]'s lifetime.
    pub fn Dart_HandleFromPersistent_DL_Trampolined(
        object: Dart_PersistentHandle,
    ) -> Dart_Handle;

    /// Deallocates the provided [`Dart_PersistentHandle`].
    pub fn Dart_DeletePersistentHandle_DL_Trampolined(
        object: Dart_PersistentHandle,
    );
}

/// Initializes usage of Dynamically Linked Dart API.
///
/// # Safety
///
/// Intended to be called ONLY with [`NativeApi.initializeApiDLData`][1] from
/// Dart.
///
/// [1]: https://api.dart.dev/dart-ffi/NativeApi/initializeApiDLData.html
#[no_mangle]
pub unsafe extern "C" fn init_dart_api_dl(
    obj: *mut libc::c_void,
) -> libc::intptr_t {
    Dart_InitializeApiDL(obj)
}

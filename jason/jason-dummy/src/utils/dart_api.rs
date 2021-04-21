//! Functionality for calling [`Dart DL API`] from Rust.
//!
//! [`Dart DL API`]: https://tinyurl.com/dart-dl-api

use dart_sys::{Dart_Handle, Dart_PersistentHandle};

#[link(name = "trampoline")]
extern "C" {
    /// Initializes dynamically linked Dart API usage. Accepts
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

    /// Allocates a [`Dart_Handle`] in the current scope from a
    /// [`Dart_PersistentHandle`].
    ///
    /// This does not affect provided [`Dart_PersistentHandle`] lifetime.
    pub fn Dart_HandleFromPersistent_DL_Trampolined(
        object: Dart_PersistentHandle,
    ) -> Dart_Handle;

    /// Deallocates provided [`Dart_PersistentHandle`].
    pub fn Dart_DeletePersistentHandle_DL_Trampolined(
        object: Dart_PersistentHandle,
    );
}

/// Called with [`NativeApi.initializeApiDLData`][1] from Dart to enable using
/// the dynamically linked Dart API.
///
/// [1]: https://api.dart.dev/dart-ffi/NativeApi/initializeApiDLData.html
#[no_mangle]
pub unsafe extern "C" fn init_dart_api_dl(
    obj: *mut libc::c_void,
) -> libc::intptr_t {
    Dart_InitializeApiDL(obj)
}

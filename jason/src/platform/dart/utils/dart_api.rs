//! Functionality for calling [`Dart DL API`] from Rust.
//!
//! [`Dart DL API`]: https://tinyurl.com/32e7fudh

use std::{ffi::c_void, ptr::NonNull};

use dart_sys::{Dart_CObject, Dart_Handle, Dart_PersistentHandle, Dart_Port};

/// TODO: We should check everything returned from API with `Dart_IsError` and
///       panic or `Dart_PropagateError`.
#[link(name = "trampoline")]
extern "C" {
    /// Initializes Dynamically Linked Dart API usage. Accepts
    /// [`NativeApi.initializeApiDLData`][1] that must be retrieved in Dart
    /// code. Must be called before calling any other Dart API method.
    ///
    /// [1]: https://api.dart.dev/dart-ffi/NativeApi/initializeApiDLData.html
    pub fn Dart_InitializeApiDL(obj: *mut c_void) -> libc::intptr_t;

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

    /// Posts a message on some port. The message will contain the
    /// [`Dart_CObject`] object graph rooted in `message`.
    ///
    /// While the message is being sent the state of the graph of
    /// [`Dart_CObject`] structures rooted in `message` should not be accessed,
    /// as the message generation will make temporary modifications to the data.
    /// When the message has been sent the graph will be fully restored.
    ///
    /// If true is returned, the message was enqueued, and finalizers for
    /// external typed data will eventually run, even if the receiving isolate
    /// shuts down before processing the message. If false is returned, the
    /// message was not enqueued and ownership of external typed data in the
    /// message remains with the caller.
    pub fn Dart_PostCObject_DL_Trampolined(
        port_id: Dart_Port,
        message: *mut Dart_CObject,
    ) -> bool;
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
    obj: NonNull<c_void>,
) -> libc::intptr_t {
    Dart_InitializeApiDL(obj.as_ptr())
}

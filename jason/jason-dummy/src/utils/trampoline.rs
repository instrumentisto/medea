use dart_sys::{Dart_Handle, Dart_PersistentHandle};

#[link(name = "trampoline")]
extern "C" {
    pub fn Dart_InitializeApiDL(obj: *mut libc::c_void) -> libc::intptr_t;
    pub fn Dart_NewPersistentHandle_DL_Trampolined(
        object: Dart_Handle,
    ) -> Dart_PersistentHandle;
    pub fn Dart_HandleFromPersistent_DL_Trampolined(
        object: Dart_PersistentHandle,
    ) -> Dart_Handle;
    pub fn Dart_DeletePersistentHandle_DL_Trampolined(
        object: Dart_PersistentHandle,
    );
    pub fn Dart_NewApiError_DL_Trampolined(
        msg: *const libc::c_char,
    ) -> Dart_Handle;
    pub fn Dart_NewUnhandledExceptionError_DL_Trampolined(
        exception: Dart_Handle,
    ) -> Dart_Handle;
    pub fn Dart_PropagateError_DL_Trampolined(handle: Dart_Handle);
}

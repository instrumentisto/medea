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
}

#[no_mangle]
pub unsafe extern "C" fn init_dart_api_dl(
    obj: *mut libc::c_void,
) -> libc::intptr_t {
    Dart_InitializeApiDL(obj)
}

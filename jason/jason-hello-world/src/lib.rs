#[link(name = "trampoline")]
extern "C" {
    fn Dart_InitializeApiDL(obj: *mut libc::c_void) -> libc::intptr_t;
    fn Dart_NewPersistentHandle_DL_Trampolined(object: Dart_Handle) -> Dart_PersistentHandle;
    fn Dart_HandleFromPersistent_DL_Trampolined(object: Dart_PersistentHandle) -> Dart_Handle;
    fn Dart_DeletePersistentHandle_DL_Trampolined(object: Dart_PersistentHandle);
}

#[no_mangle]
pub unsafe extern "C" fn InitDartApiDL(obj: *mut libc::c_void) -> libc::intptr_t {
    return Dart_InitializeApiDL(obj);
}

#[no_mangle]
pub extern "C" fn add(i: i64) -> i64 {
    i + 100
}

#[no_mangle]
pub extern "C" fn dummy_function() {}

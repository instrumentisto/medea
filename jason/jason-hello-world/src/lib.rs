use std::ffi::{CStr, CString};

pub mod audio_track_constraints;
pub mod connection_handle;
pub mod device_video_track_constraints;
pub mod input_device_info;
pub mod jason;
pub mod room_handle;
pub mod media_manager;
mod local_media_track;

use dart_sys::{
    Dart_Handle,
    Dart_PersistentHandle,
};
use crate::jason::Jason;
use crate::media_manager::MediaManager;
use crate::input_device_info::InputDeviceInfo;

#[no_mangle]
pub extern "C" fn Jason__init() -> *const Jason {
    let jason = Jason;
    Box::into_raw(Box::new(jason))
}

#[no_mangle]
pub unsafe extern "C" fn Jason__foobar(
    this: *mut Jason,
) -> *const libc::c_char {
    let this = Box::from_raw(this);
    into_dart_string(this.foobar())
}

pub unsafe extern "C" fn check_arr(
) -> *const InputDeviceInfo {
    let arr = vec![InputDeviceInfo];
    let out = arr.as_ptr();
    std::mem::forget(arr);
    out
}

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
    i + 200
}

/// strings

unsafe fn dart_string(string: *const libc::c_char) -> String {
    CStr::from_ptr(string).to_str().unwrap().to_owned()
}

unsafe fn into_dart_string(string: String) -> *const libc::c_char {
    CString::new(string).unwrap().into_raw()
}

#[no_mangle]
pub unsafe extern "C" fn Strings(string_in: *const libc::c_char) -> *const libc::c_char {
    let string_in = CStr::from_ptr(string_in).to_str().unwrap().to_owned();
    // println!("Received string from Dart: {}", string_in);
    let reversed: String = string_in.chars().into_iter().rev().collect();
    CString::new(reversed).unwrap().into_raw()
}

#[no_mangle]
pub unsafe extern "C" fn FreeRustString(s: *mut libc::c_char) {
    if s.is_null() {
        return;
    }
    CString::from_raw(s);
}

#[no_mangle]
pub extern "C" fn dummy_function() {}

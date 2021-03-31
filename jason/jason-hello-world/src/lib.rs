use std::{
    ffi::{CStr, CString},
    mem,
};

pub mod audio_track_constraints;
pub mod connection_handle;
pub mod device_video_track_constraints;
mod display_video_track_constraints;
pub mod input_device_info;
pub mod jason;
mod local_media_track;
pub mod media_manager;
mod media_stream_settings;
mod reconnect_handle;
mod remote_media_track;
mod room_close_reason;
pub mod room_handle;

use crate::{
    input_device_info::InputDeviceInfo, jason::Jason,
    media_manager::MediaManager,
};
use dart_sys::{Dart_Handle, Dart_PersistentHandle};

pub enum MediaKind {
    Foo,
}

impl Into<u8> for MediaKind {
    fn into(self) -> u8 {
        0
    }
}

pub enum MediaSourceKind {
    Foo,
}

impl Into<u8> for MediaSourceKind {
    fn into(self) -> u8 {
        0
    }
}

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

#[repr(C)]
pub struct Array<T> {
    pub len: u64,
    pub arr: *const T,
}

impl<T> Array<T> {
    pub fn new(arr: Vec<T>) -> Self {
        let len = arr.len();
        let ptr = arr.as_ptr();
        mem::forget(ptr);
        Self {
            len: len as u64,
            arr: arr.as_ptr(),
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn check_arr() -> Array<InputDeviceInfo> {
    Array::new(vec![InputDeviceInfo, InputDeviceInfo])
}

#[link(name = "trampoline")]
extern "C" {
    fn Dart_InitializeApiDL(obj: *mut libc::c_void) -> libc::intptr_t;
    fn Dart_NewPersistentHandle_DL_Trampolined(
        object: Dart_Handle,
    ) -> Dart_PersistentHandle;
    fn Dart_HandleFromPersistent_DL_Trampolined(
        object: Dart_PersistentHandle,
    ) -> Dart_Handle;
    fn Dart_DeletePersistentHandle_DL_Trampolined(
        object: Dart_PersistentHandle,
    );
}

#[no_mangle]
pub unsafe extern "C" fn InitDartApiDL(
    obj: *mut libc::c_void,
) -> libc::intptr_t {
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
pub unsafe extern "C" fn Strings(
    string_in: *const libc::c_char,
) -> *const libc::c_char {
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

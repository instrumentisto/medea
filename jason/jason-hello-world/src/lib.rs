use std::{
    convert::TryFrom as _,
    ffi::{CStr, CString},
    mem, slice,
};

pub mod audio_track_constraints;
pub mod connection_handle;
pub mod device_video_track_constraints;
pub mod display_video_track_constraints;
pub mod input_device_info;
pub mod jason;
pub mod local_media_track;
pub mod media_manager;
pub mod media_stream_settings;
pub mod reconnect_handle;
pub mod remote_media_track;
pub mod room_close_reason;
pub mod room_handle;

use dart_sys::{Dart_Handle, Dart_PersistentHandle};

use crate::{
    audio_track_constraints::AudioTrackConstraints,
    connection_handle::ConnectionHandle,
    device_video_track_constraints::{DeviceVideoTrackConstraints, FacingMode},
    display_video_track_constraints::DisplayVideoTrackConstraints,
    input_device_info::InputDeviceInfo,
    jason::Jason,
    local_media_track::LocalMediaTrack,
    media_manager::MediaManager,
    media_stream_settings::MediaStreamSettings,
    reconnect_handle::ReconnectHandle,
    remote_media_track::RemoteMediaTrack,
    room_close_reason::RoomCloseReason,
    room_handle::RoomHandle,
};

#[no_mangle]
pub extern "C" fn dummy_function() {}

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
    pub arr: *const *mut T,
}

impl<T> Array<T> {
    pub fn new(arr: Vec<T>) -> Self {
        let out: Vec<_> = arr
            .into_iter()
            .map(|e| Box::into_raw(Box::new(e)))
            .collect();
        Self {
            len: out.len() as u64,
            arr: Box::leak(out.into_boxed_slice()).as_ptr(),
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn check_arr() -> Array<InputDeviceInfo> {
    let a = InputDeviceInfo { foo: 100, bar: 100 };
    Array::new(vec![
        a,
        InputDeviceInfo { foo: 100, bar: 200 },
        InputDeviceInfo { foo: 300, bar: 400 },
        InputDeviceInfo { foo: 500, bar: 600 },
    ])
}

#[no_mangle]
pub unsafe extern "C" fn InputDeviceInfo__device_id(
    this: *mut InputDeviceInfo,
) -> *const libc::c_char {
    let this = Box::from_raw(this);
    into_dart_string(this.device_id())
}

#[no_mangle]
pub unsafe extern "C" fn free_array(arr: Array<InputDeviceInfo>) {
    drop(arr);
}

impl<T> Drop for Array<T> {
    fn drop(&mut self) {
        unsafe {
            slice::from_raw_parts_mut(self.arr as *mut i64, self.len as usize);
        };
    }
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
pub unsafe extern "C" fn RoomCloseReason__reason(
    this: *mut RoomCloseReason,
) -> *const libc::c_char {
    let this = Box::from_raw(this);
    into_dart_string(this.reason())
}

#[no_mangle]
pub unsafe extern "C" fn RoomCloseReason__is_closed_by_server(
    this: *mut RoomCloseReason,
) -> bool {
    let this = Box::from_raw(this);
    this.is_closed_by_server()
}

#[no_mangle]
pub unsafe extern "C" fn RoomCloseReason__is_err(
    this: *mut RoomCloseReason,
) -> bool {
    let this = Box::from_raw(this);
    this.is_err()
}

#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__enable(this: *mut RemoteMediaTrack) {
    let this = Box::from_raw(this);
    this.enable();
}

#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__kind(
    this: *mut RemoteMediaTrack,
) -> u8 {
    let this = Box::from_raw(this);
    this.kind().into()
}

#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__media_source_kind(
    this: *mut RemoteMediaTrack,
) -> u8 {
    let this = Box::from_raw(this);
    this.media_source_kind().into()
}

#[no_mangle]
pub unsafe extern "C" fn ReconnectHandle__reconnect_with_delay(
    this: *mut ReconnectHandle,
    delay_ms: u32,
) {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn MediaStreamSettings__audio(
    this: *mut MediaStreamSettings,
    constraints: *mut AudioTrackConstraints,
) {
    let this = Box::from_raw(this);
    let constraints = Box::from_raw(constraints);
    this.audio(&constraints);
}

#[no_mangle]
pub unsafe extern "C" fn MediaStreamSettings__device_video(
    this: *mut MediaStreamSettings,
    constraints: *mut DeviceVideoTrackConstraints,
) {
    let this = Box::from_raw(this);
    let constraints = Box::from_raw(constraints);
    this.device_video(&constraints);
}

#[no_mangle]
pub unsafe extern "C" fn MediaStreamSettings__display_video(
    this: *mut MediaStreamSettings,
    constraints: *mut DisplayVideoTrackConstraints,
) {
    let this = Box::from_raw(this);
    let constraints = Box::from_raw(constraints);
    this.display_video(&constraints);
}

#[no_mangle]
pub unsafe extern "C" fn MediaManager__init_local_tracks(
    this: *mut MediaManager,
) -> Array<LocalMediaTrack> {
    let this = Box::from_raw(this);
    Array::new(this.init_local_tracks())
}

#[no_mangle]
pub unsafe extern "C" fn LocalMediaTrack__kind(
    this: *mut LocalMediaTrack,
) -> u8 {
    let this = Box::from_raw(this);
    this.kind().into()
}

#[no_mangle]
pub unsafe extern "C" fn LocalMediaTrack__media_source_kind(
    this: *mut LocalMediaTrack,
) -> u8 {
    let this = Box::from_raw(this);
    this.media_source_kind().into()
}

#[no_mangle]
pub unsafe extern "C" fn Jason__init_room(this: *mut Jason) -> *mut RoomHandle {
    let this = Box::from_raw(this);
    Box::into_raw(Box::new(this.init_room()))
}

#[no_mangle]
pub unsafe extern "C" fn Jason__media_manager(
    this: *mut Jason,
) -> *mut MediaManager {
    let this = Box::from_raw(this);
    Box::into_raw(Box::new(this.media_manager()))
}

#[no_mangle]
pub unsafe extern "C" fn Jason__close_room(
    this: *mut Jason,
    room_to_delete: *mut RoomHandle,
) {
    let this = Box::from_raw(this);
    let room_to_delete = Box::from_raw(room_to_delete);
    this.close_room(&room_to_delete);
}

#[no_mangle]
pub unsafe extern "C" fn InputDeviceInfo__kind(
    this: *mut InputDeviceInfo,
) -> u8 {
    let this = Box::from_raw(this);
    this.kind().into()
}

#[no_mangle]
pub unsafe extern "C" fn InputDeviceInfo__label<'a>(
    this: *mut InputDeviceInfo,
) -> *const libc::c_char {
    let this = Box::from_raw(this);
    into_dart_string(this.label())
}

#[no_mangle]
pub unsafe extern "C" fn InputDeviceInfo_nativeGroupId<'a>(
    this: *mut InputDeviceInfo,
) -> *const libc::c_char {
    let this = Box::from_raw(this);
    into_dart_string(this.group_id())
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__device_id(
    this: *mut DeviceVideoTrackConstraints,
    device_id: *const libc::c_char,
) {
    let mut this = Box::from_raw(this);
    this.device_id(dart_string(device_id));
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__exact_facing_mode(
    this: *mut DeviceVideoTrackConstraints,
    facing_mode: u8,
) {
    let mut this = Box::from_raw(this);
    let facing_mode = FacingMode::try_from(facing_mode).unwrap();
    this.exact_facing_mode(facing_mode);
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__ideal_facing_mode(
    this: *mut DeviceVideoTrackConstraints,
    facing_mode: u8,
) {
    let mut this = Box::from_raw(this);
    let facing_mode = FacingMode::try_from(facing_mode).unwrap();
    this.ideal_facing_mode(facing_mode);
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__exact_height(
    this: *mut DeviceVideoTrackConstraints,
    height: u32,
) {
    let mut this = Box::from_raw(this);
    this.exact_height(height);
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraits__ideal_height(
    this: *mut DeviceVideoTrackConstraints,
    height: u32,
) {
    let mut this = Box::from_raw(this);
    this.ideal_height(height);
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__height_in_range(
    this: *mut DeviceVideoTrackConstraints,
    min: u32,
    max: u32,
) {
    let mut this = Box::from_raw(this);
    this.height_in_range(min, max);
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__exact_width(
    this: *mut DeviceVideoTrackConstraints,
    width: u32,
) {
    let mut this = Box::from_raw(this);
    this.exact_width(width);
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraits__ideal_width(
    this: *mut DeviceVideoTrackConstraints,
    width: u32,
) {
    let mut this = Box::from_raw(this);
    this.ideal_width(width);
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__width_in_range(
    this: *mut DeviceVideoTrackConstraints,
    min: u32,
    max: u32,
) {
    let mut this = Box::from_raw(this);
    this.width_in_range(min, max);
}

#[no_mangle]
pub unsafe extern "C" fn ConnectionHandle__get_remote_member_id(
    this: *mut ConnectionHandle,
) -> *const libc::c_char {
    let this = Box::from_raw(this);
    into_dart_string(this.get_remote_member_id())
}

#[no_mangle]
pub unsafe extern "C" fn AudioTrackConstraints__native_device_id(
    this: *mut AudioTrackConstraints,
    device_id: *const libc::c_char,
) {
    let mut this = Box::from_raw(this);
    // TODO: drop strings on Dart side
    this.native_device_id(unsafe { dart_string(device_id) })
}

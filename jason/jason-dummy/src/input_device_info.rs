use crate::{
    utils::{ptr_from_dart_as_mut, string_into_c_str},
    MediaKind,
};

pub struct InputDeviceInfo {}

impl InputDeviceInfo {
    pub fn device_id(&self) -> String {
        String::from("InputDeviceInfo.device_id")
    }

    pub fn kind(&self) -> MediaKind {
        MediaKind::Audio
    }

    pub fn label(&self) -> String {
        String::from("InputDeviceInfo.label")
    }

    pub fn group_id(&self) -> String {
        String::from("InputDeviceInfo.group_id")
    }
}

#[no_mangle]
pub unsafe extern "C" fn InputDeviceInfo__device_id(
    this: *mut InputDeviceInfo,
) -> *const libc::c_char {
    let device_id = ptr_from_dart_as_mut(this).device_id();
    string_into_c_str(device_id)
}

#[no_mangle]
pub unsafe extern "C" fn InputDeviceInfo__kind(
    this: *mut InputDeviceInfo,
) -> u8 {
    ptr_from_dart_as_mut(this).kind() as u8
}

#[no_mangle]
pub unsafe extern "C" fn InputDeviceInfo__label(
    this: *mut InputDeviceInfo,
) -> *const libc::c_char {
    let label = ptr_from_dart_as_mut(this).label();
    string_into_c_str(label)
}

#[no_mangle]
pub unsafe extern "C" fn InputDeviceInfo_nativeGroupId(
    this: *mut InputDeviceInfo,
) -> *const libc::c_char {
    let group_id = ptr_from_dart_as_mut(this).group_id();
    string_into_c_str(group_id)
}

#[no_mangle]
pub unsafe extern "C" fn InputDeviceInfo__free(this: *mut InputDeviceInfo) {
    Box::from_raw(this);
}

use crate::{
    utils::{ptr_from_dart_as_ref, string_into_c_str},
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
    this: *const InputDeviceInfo,
) -> *const libc::c_char {
    let this = ptr_from_dart_as_ref(this);

    string_into_c_str(this.device_id())
}

#[no_mangle]
pub unsafe extern "C" fn InputDeviceInfo__kind(
    this: *const InputDeviceInfo,
) -> u8 {
    let this = ptr_from_dart_as_ref(this);

    this.kind() as u8
}

#[no_mangle]
pub unsafe extern "C" fn InputDeviceInfo__label(
    this: *const InputDeviceInfo,
) -> *const libc::c_char {
    let this = ptr_from_dart_as_ref(this);

    string_into_c_str(this.label())
}

#[no_mangle]
pub unsafe extern "C" fn InputDeviceInfo__group_id(
    this: *const InputDeviceInfo,
) -> *const libc::c_char {
    let this = ptr_from_dart_as_ref(this);

    string_into_c_str(this.group_id())
}

#[no_mangle]
pub unsafe extern "C" fn InputDeviceInfo__free(this: *mut InputDeviceInfo) {
    if !this.is_null() {
        Box::from_raw(this);
    }
}

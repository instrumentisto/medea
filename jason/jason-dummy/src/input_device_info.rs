use crate::{utils::string_into_c_str, MediaKind};

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
    let this = this.as_ref().unwrap();

    string_into_c_str(this.device_id())
}

#[no_mangle]
pub unsafe extern "C" fn InputDeviceInfo__kind(
    this: *const InputDeviceInfo,
) -> u8 {
    let this = this.as_ref().unwrap();

    this.kind() as u8
}

#[no_mangle]
pub unsafe extern "C" fn InputDeviceInfo__label(
    this: *const InputDeviceInfo,
) -> *const libc::c_char {
    let this = this.as_ref().unwrap();

    string_into_c_str(this.label())
}

#[no_mangle]
pub unsafe extern "C" fn InputDeviceInfo__group_id(
    this: *const InputDeviceInfo,
) -> *const libc::c_char {
    let this = this.as_ref().unwrap();

    string_into_c_str(this.group_id())
}

#[no_mangle]
pub unsafe extern "C" fn InputDeviceInfo__free(this: *mut InputDeviceInfo) {
    Box::from_raw(this);
}

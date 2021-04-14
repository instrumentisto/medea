use dart_sys::Dart_Handle;

use crate::{media::MediaKind, utils::dart::from_dart_string};

type DeviceIdFunction = extern "C" fn(Dart_Handle) -> *const libc::c_char;
static mut DEVICE_ID_FUNCTION: Option<DeviceIdFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_InputDeviceInfo__device_id(
    f: DeviceIdFunction,
) {
    DEVICE_ID_FUNCTION = Some(f);
}

type LabelFunction = extern "C" fn(Dart_Handle) -> *const libc::c_char;
static mut LABEL_FUNCTION: Option<LabelFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_InputDeviceInfo__label(f: LabelFunction) {
    LABEL_FUNCTION = Some(f);
}

type GroupIdFunction = extern "C" fn(Dart_Handle) -> *const libc::c_char;
static mut GROUP_ID_FUNCTION: Option<GroupIdFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_InputDeviceInfo__group_id(
    f: GroupIdFunction,
) {
    GROUP_ID_FUNCTION = Some(f);
}

type KindFunction = extern "C" fn(Dart_Handle) -> i32;
static mut KIND_FUNCTION: Option<KindFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_InputDeviceInfo__kind(f: KindFunction) {
    KIND_FUNCTION = Some(f);
}

impl From<i32> for MediaKind {
    fn from(i: i32) -> Self {
        // TODO: recheck it
        match i {
            0 => Self::Audio,
            1 => Self::Video,
            _ => unreachable!("Unknown MediaKind enum variant"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct InputDeviceInfo {
    media_kind: MediaKind,
    info: Dart_Handle,
}

impl InputDeviceInfo {
    pub(super) fn new(handle: Dart_Handle) -> Self {
        todo!("See todo after this line");
        Self {
            // TODO: Provide real MediaKind here
            media_kind: MediaKind::Audio,
            info: handle,
        }
    }

    pub fn device_id(&self) -> String {
        unsafe { from_dart_string(DEVICE_ID_FUNCTION.unwrap()(self.info)) }
    }

    pub fn label(&self) -> String {
        unsafe { from_dart_string(LABEL_FUNCTION.unwrap()(self.info)) }
    }

    pub fn group_id(&self) -> String {
        unsafe { from_dart_string(GROUP_ID_FUNCTION.unwrap()(self.info)) }
    }

    pub fn kind(&self) -> MediaKind {
        unsafe { KIND_FUNCTION.unwrap()(self.info).into() }
    }
}

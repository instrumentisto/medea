use dart_sys::Dart_Handle;

use crate::{media::MediaKind, utils::dart::from_dart_string};

type DeviceIdFunction = extern "C" fn(Dart_Handle) -> *const libc::c_char;
static mut device_id_function: Option<DeviceIdFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_InputDeviceInfo__device_id(
    f: DeviceIdFunction,
) {
    device_id_function = Some(f);
}

type LabelFunction = extern "C" fn(Dart_Handle) -> *const libc::c_char;
static mut label_function: Option<LabelFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_InputDeviceInfo__label(f: LabelFunction) {
    label_function = Some(f);
}

type GroupIdFunction = extern "C" fn(Dart_Handle) -> *const libc::c_char;
static mut group_id_function: Option<GroupIdFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_InputDeviceInfo__group_id(
    f: GroupIdFunction,
) {
    group_id_function = Some(f);
}

type KindFunction = extern "C" fn(Dart_Handle) -> i32;
static mut kind_function: Option<KindFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_InputDeviceInfo__kind(f: KindFunction) {
    kind_function = Some(f);
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
        unsafe { from_dart_string(device_id_function.unwrap()(self.info)) }
    }

    pub fn label(&self) -> String {
        unsafe { from_dart_string(label_function.unwrap()(self.info)) }
    }

    pub fn group_id(&self) -> String {
        unsafe { from_dart_string(group_id_function.unwrap()(self.info)) }
    }

    pub fn kind(&self) -> MediaKind {
        unsafe { kind_function.unwrap()(self.info).into() }
    }
}

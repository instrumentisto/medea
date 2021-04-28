use dart_sys::Dart_Handle;

use crate::{
    media::MediaKind,
    platform::dart::utils::{handle::DartHandle, option::DartStringOption},
};

type DeviceIdFunction = extern "C" fn(Dart_Handle) -> DartStringOption;
static mut DEVICE_ID_FUNCTION: Option<DeviceIdFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_InputDeviceInfo__device_id(
    f: DeviceIdFunction,
) {
    DEVICE_ID_FUNCTION = Some(f);
}

type LabelFunction = extern "C" fn(Dart_Handle) -> DartStringOption;
static mut LABEL_FUNCTION: Option<LabelFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_InputDeviceInfo__label(f: LabelFunction) {
    LABEL_FUNCTION = Some(f);
}

type GroupIdFunction = extern "C" fn(Dart_Handle) -> DartStringOption;
static mut GROUP_ID_FUNCTION: Option<GroupIdFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_InputDeviceInfo__group_id(
    f: GroupIdFunction,
) {
    GROUP_ID_FUNCTION = Some(f);
}

type KindFunction = extern "C" fn(Dart_Handle) -> DartStringOption;
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
    info: DartHandle,
}

impl From<DartHandle> for InputDeviceInfo {
    fn from(_handle: DartHandle) -> Self {
        todo!("Provide real MediaKind")
    }
}

impl InputDeviceInfo {
    pub fn device_id(&self) -> String {
        // Device ID should be always Some
        unsafe {
            Option::from(DEVICE_ID_FUNCTION.unwrap()(self.info.get())).unwrap()
        }
    }

    pub fn label(&self) -> String {
        // Label should be always Some
        unsafe {
            Option::from(LABEL_FUNCTION.unwrap()(self.info.get())).unwrap()
        }
    }

    pub fn group_id(&self) -> String {
        // Group ID should be always Some
        unsafe {
            Option::from(GROUP_ID_FUNCTION.unwrap()(self.info.get())).unwrap()
        }
    }

    pub fn kind(&self) -> MediaKind {
        unsafe {
            // Kind should be always Some
            let kind: String =
                Option::from(KIND_FUNCTION.unwrap()(self.info.get())).unwrap();
            kind.parse().unwrap()
        }
    }
}

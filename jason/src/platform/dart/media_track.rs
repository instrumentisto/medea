use dart_sys::Dart_Handle;
use derive_more::From;

use crate::{
    media::{track::MediaStreamTrackState, FacingMode, MediaKind},
    platform::dart::utils::callback::VoidCallback,
    utils::dart::from_dart_string,
};

#[derive(Clone, From, Debug)]
pub struct MediaStreamTrack(Dart_Handle);

type IdFunction = extern "C" fn(Dart_Handle) -> *const libc::c_char;
static mut ID_FUNCTION: Option<IdFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_MediaStreamTrack__id(f: IdFunction) {
    ID_FUNCTION = Some(f);
}

type DeviceIdFunction = extern "C" fn(Dart_Handle) -> *const libc::c_char;
static mut DEVICE_ID_FUNCTION: Option<DeviceIdFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_MediaStreamTrack__device_id(
    f: DeviceIdFunction,
) {
    DEVICE_ID_FUNCTION = Some(f);
}

type FacingModeFunction = extern "C" fn(Dart_Handle) -> i32;
static mut FACING_MODE_FUNCTION: Option<FacingModeFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_MediaStreamTrack__facing_mode(
    f: FacingModeFunction,
) {
    FACING_MODE_FUNCTION = Some(f);
}

type HeightFunction = extern "C" fn(Dart_Handle) -> i32;
static mut HEIGHT_FUNCTION: Option<HeightFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_MediaStreamTrack__height(f: HeightFunction) {
    HEIGHT_FUNCTION = Some(f);
}

type WidthFunction = extern "C" fn(Dart_Handle) -> i32;
static mut WIDTH_FUNCTION: Option<WidthFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_MediaStreamTrack__width(f: WidthFunction) {
    WIDTH_FUNCTION = Some(f);
}

type SetEnabledFunction = extern "C" fn(Dart_Handle, bool);
static mut SET_ENABLED_FUNCTION: Option<SetEnabledFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_MediaStreamTrack__set_enabled(
    f: SetEnabledFunction,
) {
    SET_ENABLED_FUNCTION = Some(f);
}

type StopFunction = extern "C" fn(Dart_Handle);
static mut STOP_FUNCTION: Option<StopFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_MediaStreamTrack__stop(f: StopFunction) {
    STOP_FUNCTION = Some(f);
}

type EnabledFunction = extern "C" fn(Dart_Handle) -> bool;
static mut ENABLED_FUNCTION: Option<EnabledFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_MediaStreamTrack__enabled(
    f: EnabledFunction,
) {
    ENABLED_FUNCTION = Some(f);
}

type KindFunction = extern "C" fn(Dart_Handle) -> i32;
static mut KIND_FUNCTION: Option<KindFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_MediaStreamTrack__kind(f: KindFunction) {
    KIND_FUNCTION = Some(f);
}

type ReadyStateFunction = extern "C" fn(Dart_Handle) -> i32;
static mut READY_STATE_FUNCTION: Option<ReadyStateFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_MediaStreamTrack__ready_state(
    f: ReadyStateFunction,
) {
    READY_STATE_FUNCTION = Some(f);
}

type OnEndedFunction = extern "C" fn(Dart_Handle, Dart_Handle);
static mut ON_ENDED_FUNCTION: Option<OnEndedFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_MediaStreamTrack__on_ended(
    f: OnEndedFunction,
) {
    ON_ENDED_FUNCTION = Some(f);
}

impl MediaStreamTrack {
    pub fn id(&self) -> String {
        unsafe { from_dart_string(ID_FUNCTION.unwrap()(self.0)) }
    }

    pub fn kind(&self) -> MediaKind {
        MediaKind::from(unsafe { KIND_FUNCTION.unwrap()(self.0) })
    }

    pub fn ready_state(&self) -> MediaStreamTrackState {
        MediaStreamTrackState::from(unsafe {
            READY_STATE_FUNCTION.unwrap()(self.0)
        })
    }

    pub fn device_id(&self) -> Option<String> {
        unsafe {
            let device_id = DEVICE_ID_FUNCTION.unwrap()(self.0);
            if device_id.is_null() {
                None
            } else {
                Some(from_dart_string(device_id))
            }
        }
    }

    pub fn facing_mode(&self) -> Option<FacingMode> {
        unsafe {
            let facing_mode = FACING_MODE_FUNCTION.unwrap()(self.0);
            // TODO: maybe it's needs to be nullable?
            Some(FacingMode::from(facing_mode))
        }
    }

    pub fn height(&self) -> Option<u32> {
        unsafe {
            // TODO: maybe it's needs to be nullable?
            Some(HEIGHT_FUNCTION.unwrap()(self.0) as u32)
        }
    }

    pub fn width(&self) -> Option<u32> {
        unsafe {
            // TODO: maybe it's needs to be nullable?
            Some(WIDTH_FUNCTION.unwrap()(self.0) as u32)
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        unsafe {
            SET_ENABLED_FUNCTION.unwrap()(self.0, enabled);
        }
    }

    pub fn stop(&self) {
        unsafe {
            STOP_FUNCTION.unwrap()(self.0);
        }
    }

    pub fn enabled(&self) -> bool {
        unsafe { ENABLED_FUNCTION.unwrap()(self.0) }
    }

    pub fn guess_is_from_display(&self) -> bool {
        todo!()
    }

    pub fn fork(&self) -> Self {
        unimplemented!()
    }

    pub fn on_ended<F>(&self, f: Option<F>)
    where
        F: 'static + FnOnce(),
    {
        if let Some(cb) = f {
            let cb = VoidCallback::callback(cb);
            unsafe { ON_ENDED_FUNCTION.unwrap()(self.0, cb) };
        }
    }
}

use dart_sys::Dart_Handle;
use derive_more::From;

use crate::{
    media::{track::MediaStreamTrackState, FacingMode, MediaKind},
    platform::dart::utils::{
        callback_listener::VoidCallback,
        handle::DartHandle,
        option::{DartIntOption, DartStringOption, DartUIntOption},
    },
    utils::dart::from_dart_string,
};

type IdFunction = extern "C" fn(Dart_Handle) -> *const libc::c_char;

type DeviceIdFunction = extern "C" fn(Dart_Handle) -> DartStringOption;

type FacingModeFunction = extern "C" fn(Dart_Handle) -> DartIntOption;

type HeightFunction = extern "C" fn(Dart_Handle) -> DartUIntOption;

type WidthFunction = extern "C" fn(Dart_Handle) -> DartUIntOption;

type SetEnabledFunction = extern "C" fn(Dart_Handle, bool);

type StopFunction = extern "C" fn(Dart_Handle);

type EnabledFunction = extern "C" fn(Dart_Handle) -> bool;

type KindFunction = extern "C" fn(Dart_Handle) -> i32;

type ReadyStateFunction = extern "C" fn(Dart_Handle) -> i32;

type OnEndedFunction = extern "C" fn(Dart_Handle, Dart_Handle);

static mut ID_FUNCTION: Option<IdFunction> = None;

static mut DEVICE_ID_FUNCTION: Option<DeviceIdFunction> = None;

static mut FACING_MODE_FUNCTION: Option<FacingModeFunction> = None;

static mut HEIGHT_FUNCTION: Option<HeightFunction> = None;

static mut WIDTH_FUNCTION: Option<WidthFunction> = None;

static mut SET_ENABLED_FUNCTION: Option<SetEnabledFunction> = None;

static mut STOP_FUNCTION: Option<StopFunction> = None;

static mut ENABLED_FUNCTION: Option<EnabledFunction> = None;

static mut KIND_FUNCTION: Option<KindFunction> = None;

static mut READY_STATE_FUNCTION: Option<ReadyStateFunction> = None;

static mut ON_ENDED_FUNCTION: Option<OnEndedFunction> = None;

#[derive(Clone, From, Debug)]
pub struct MediaStreamTrack(DartHandle);

impl From<Dart_Handle> for MediaStreamTrack {
    fn from(handle: Dart_Handle) -> Self {
        Self(DartHandle::new(handle))
    }
}

#[no_mangle]
pub unsafe extern "C" fn register_MediaStreamTrack__id(f: IdFunction) {
    ID_FUNCTION = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn register_MediaStreamTrack__device_id(
    f: DeviceIdFunction,
) {
    DEVICE_ID_FUNCTION = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn register_MediaStreamTrack__facing_mode(
    f: FacingModeFunction,
) {
    FACING_MODE_FUNCTION = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn register_MediaStreamTrack__height(f: HeightFunction) {
    HEIGHT_FUNCTION = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn register_MediaStreamTrack__width(f: WidthFunction) {
    WIDTH_FUNCTION = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn register_MediaStreamTrack__set_enabled(
    f: SetEnabledFunction,
) {
    SET_ENABLED_FUNCTION = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn register_MediaStreamTrack__stop(f: StopFunction) {
    STOP_FUNCTION = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn register_MediaStreamTrack__enabled(
    f: EnabledFunction,
) {
    ENABLED_FUNCTION = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn register_MediaStreamTrack__kind(f: KindFunction) {
    KIND_FUNCTION = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn register_MediaStreamTrack__ready_state(
    f: ReadyStateFunction,
) {
    READY_STATE_FUNCTION = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn register_MediaStreamTrack__on_ended(
    f: OnEndedFunction,
) {
    ON_ENDED_FUNCTION = Some(f);
}

impl MediaStreamTrack {
    pub fn track(&self) -> Dart_Handle {
        self.0.get()
    }

    pub fn id(&self) -> String {
        unsafe { from_dart_string(ID_FUNCTION.unwrap()(self.0.get())) }
    }

    pub fn kind(&self) -> MediaKind {
        MediaKind::from(unsafe { KIND_FUNCTION.unwrap()(self.0.get()) })
    }

    pub fn ready_state(&self) -> MediaStreamTrackState {
        MediaStreamTrackState::from(unsafe {
            READY_STATE_FUNCTION.unwrap()(self.0.get())
        })
    }

    pub fn device_id(&self) -> Option<String> {
        unsafe { DEVICE_ID_FUNCTION.unwrap()(self.0.get()).into() }
    }

    pub fn facing_mode(&self) -> Option<FacingMode> {
        unsafe {
            let facing_mode: i32 =
                Option::from(FACING_MODE_FUNCTION.unwrap()(self.0.get()))?;
            Some(FacingMode::from(facing_mode))
        }
    }

    pub fn height(&self) -> Option<u32> {
        unsafe { HEIGHT_FUNCTION.unwrap()(self.0.get()).into() }
    }

    pub fn width(&self) -> Option<u32> {
        unsafe { WIDTH_FUNCTION.unwrap()(self.0.get()).into() }
    }

    pub fn set_enabled(&self, enabled: bool) {
        unsafe {
            SET_ENABLED_FUNCTION.unwrap()(self.0.get(), enabled);
        }
    }

    pub fn stop(&self) {
        unsafe {
            STOP_FUNCTION.unwrap()(self.0.get());
        }
    }

    pub fn enabled(&self) -> bool {
        unsafe { ENABLED_FUNCTION.unwrap()(self.0.get()) }
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
            unsafe { ON_ENDED_FUNCTION.unwrap()(self.0.get(), cb) };
        }
    }
}
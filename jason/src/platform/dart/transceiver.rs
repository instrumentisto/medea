use std::rc::Rc;

use dart_sys::{Dart_Handle, _Dart_Handle};

use crate::{media::track::local, platform::TransceiverDirection};

#[derive(Clone, Debug)]
pub struct Transceiver {
    transceiver: Dart_Handle,
}

impl From<Dart_Handle> for Transceiver {
    fn from(handle: *mut _Dart_Handle) -> Self {
        Self {
            transceiver: handle,
        }
    }
}

type CurrentDirectionFunction = extern "C" fn(Dart_Handle) -> i32;
static mut CURRENT_DIRECTION_FUNCTION: Option<CurrentDirectionFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_Transceiver__current_direction(
    f: CurrentDirectionFunction,
) {
    CURRENT_DIRECTION_FUNCTION = Some(f);
}

type SetSendTrackFunction = extern "C" fn(Dart_Handle) -> Dart_Handle;
static mut GET_SEND_TRACK_FUNCTION: Option<SetSendTrackFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_Transceiver__set_send_track(
    f: SetSendTrackFunction,
) {
    GET_SEND_TRACK_FUNCTION = Some(f);
}

type ReplaceTrackFunction = extern "C" fn(Dart_Handle, Dart_Handle);
static mut REPLACE_TRACK_FUNCTION: Option<ReplaceTrackFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_Transceiver__replace_track(
    f: ReplaceTrackFunction,
) {
    REPLACE_TRACK_FUNCTION = Some(f);
}

type DropSenderFunction = extern "C" fn(Dart_Handle);
static mut DROP_SENDER_FUNCTION: Option<DropSenderFunction> = None;

type SetSenderTrackEnabledFunction = extern "C" fn(Dart_Handle, bool);
static mut SET_SENDER_TRACK_ENABLED_FUNCTION: Option<
    SetSenderTrackEnabledFunction,
> = None;

#[no_mangle]
pub unsafe extern "C" fn register_Transceiver__set_sender_track_enabled(
    f: SetSenderTrackEnabledFunction,
) {
    SET_SENDER_TRACK_ENABLED_FUNCTION = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn register_Transceiver__drop_sender(
    f: DropSenderFunction,
) {
    DROP_SENDER_FUNCTION = Some(f);
}

type IsStoppedFunction = extern "C" fn(Dart_Handle) -> bool;
static mut IS_STOPPED_FUNCTION: Option<IsStoppedFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_Transceiver__is_stopped(
    f: IsStoppedFunction,
) {
    IS_STOPPED_FUNCTION = Some(f);
}

impl Transceiver {
    pub fn current_direction(&self) -> TransceiverDirection {
        unsafe { CURRENT_DIRECTION_FUNCTION.unwrap()(self.transceiver).into() }
    }

    pub fn sub_direction(&self, disabled_direction: TransceiverDirection) {
        todo!()
    }

    pub fn add_direction(&self, enabled_direction: TransceiverDirection) {
        todo!()
    }

    pub fn has_direction(&self, direction: TransceiverDirection) -> bool {
        todo!()
    }

    // TODO: replace Dart_Handle with local::Track
    pub fn set_send_track(&self, new_sender: Dart_Handle) {
        unsafe {
            let sender = GET_SEND_TRACK_FUNCTION.unwrap()(self.transceiver);
            REPLACE_TRACK_FUNCTION.unwrap()(sender, new_sender);
        }
    }

    pub async fn drop_send_track(&self) {
        unsafe {
            let sender = GET_SEND_TRACK_FUNCTION.unwrap()(self.transceiver);
            DROP_SENDER_FUNCTION.unwrap()(sender);
        }
    }

    pub fn set_send_track_enabled(&self, enabled: bool) {
        unsafe {
            let sender = GET_SEND_TRACK_FUNCTION.unwrap()(self.transceiver);
            SET_SENDER_TRACK_ENABLED_FUNCTION.unwrap()(sender, enabled);
        }
    }

    pub fn is_stopped(&self) -> bool {
        unsafe { IS_STOPPED_FUNCTION.unwrap()(self.transceiver) }
    }

    pub fn mid(&self) -> Option<String> {
        todo!()
    }

    pub fn send_track(&self) -> Option<Rc<local::Track>> {
        todo!()
    }

    pub fn has_send_track(&self) -> bool {
        todo!()
    }
}

use crate::{media::track::local, platform::TransceiverDirection};
use dart_sys::Dart_Handle;
use std::rc::Rc;

#[derive(Clone, Debug)]
pub struct Transceiver {
    transceiver: Dart_Handle,
}

type CurrentDirectionFunction = extern "C" fn(Dart_Handle) -> i32;
static mut current_direction_function: Option<CurrentDirectionFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_Transceiver__current_direction(
    f: CurrentDirectionFunction,
) {
    current_direction_function = Some(f);
}

type SetSendTrackFunction = extern "C" fn(Dart_Handle) -> Dart_Handle;
static mut get_send_track_function: Option<SetSendTrackFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_Transceiver__set_send_track(
    f: SetSendTrackFunction,
) {
    get_send_track_function = Some(f);
}

type ReplaceTrackFunction = extern "C" fn(Dart_Handle, Dart_Handle);
static mut replace_track_function: Option<ReplaceTrackFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_Transceiver__replace_track(
    f: ReplaceTrackFunction,
) {
    replace_track_function = Some(f);
}

type DropSenderFunction = extern "C" fn(Dart_Handle);
static mut drop_sender_function: Option<DropSenderFunction> = None;

type SetSenderTrackEnabledFunction = extern "C" fn(Dart_Handle, bool);
static mut set_sender_track_enabled_function: Option<
    SetSenderTrackEnabledFunction,
> = None;

#[no_mangle]
pub unsafe extern "C" fn register_Transceiver__set_sender_track_enabled(
    f: SetSenderTrackEnabledFunction,
) {
    set_sender_track_enabled_function = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn register_Transceiver__drop_sender(
    f: DropSenderFunction,
) {
    drop_sender_function = Some(f);
}

type IsStoppedFunction = extern "C" fn(Dart_Handle) -> bool;
static mut is_stopped_function: Option<IsStoppedFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_Transceiver__is_stopped(
    f: IsStoppedFunction,
) {
    is_stopped_function = Some(f);
}

impl Transceiver {
    pub fn current_direction(&self) -> TransceiverDirection {
        unsafe { current_direction_function.unwrap()(self.transceiver).into() }
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
            let sender = get_send_track_function.unwrap()(self.transceiver);
            replace_track_function.unwrap()(sender, new_sender);
        }
    }

    pub async fn drop_send_track(&self) {
        unsafe {
            let sender = get_send_track_function.unwrap()(self.transceiver);
            drop_sender_function.unwrap()(sender);
        }
    }

    pub fn set_send_track_enabled(&self, enabled: bool) {
        unsafe {
            let sender = get_send_track_function.unwrap()(self.transceiver);
            set_sender_track_enabled_function.unwrap()(sender, enabled);
        }
    }

    pub fn is_stopped(&self) -> bool {
        unsafe { is_stopped_function.unwrap()(self.transceiver) }
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

use std::{future::Future, rc::Rc};

use dart_sys::Dart_Handle;
use futures::future::LocalBoxFuture;
use medea_client_api_proto::MediaSourceKind;

use crate::{
    media::track::local,
    platform,
    platform::{
        dart::utils::{
            handle::DartHandle,
            option::{DartOption, DartStringOption},
        },
        MediaStreamTrack, TransceiverDirection,
    },
    utils::dart::dart_future::DartFuture,
};
use crate::utils::dart::dart_future::VoidDartFuture;

#[derive(Clone, Debug)]
pub struct Transceiver {
    transceiver: DartHandle,
}

impl From<DartHandle> for Transceiver {
    fn from(handle: DartHandle) -> Self {
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

type GetSendTrackFunction = extern "C" fn(Dart_Handle) -> DartOption;
static mut GET_SEND_TRACK_FUNCTION: Option<GetSendTrackFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_Transceiver__get_send_track(
    f: GetSendTrackFunction,
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

type SetSendTrackEnabledFunction = extern "C" fn(Dart_Handle, bool);
static mut SET_SEND_TRACK_ENABLED_FUNCTION: Option<
    SetSendTrackEnabledFunction,
> = None;

#[no_mangle]
pub unsafe extern "C" fn register_Transceiver__set_send_track_enabled(
    f: SetSendTrackEnabledFunction,
) {
    SET_SEND_TRACK_ENABLED_FUNCTION = Some(f);
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

type MidFunction = extern "C" fn(Dart_Handle) -> DartStringOption;
static mut MID_FUNCTION: Option<MidFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_Transceiver__mid(f: MidFunction) {
    MID_FUNCTION = Some(f)
}

type SendTrackFunction = extern "C" fn(Dart_Handle) -> DartOption;
static mut SEND_TRACK_FUNCTION: Option<SendTrackFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_Transceiver__send_track(
    f: SendTrackFunction,
) {
    SEND_TRACK_FUNCTION = Some(f);
}

type HasSendTrackFunction = extern "C" fn(Dart_Handle) -> i8;
static mut HAS_SEND_TRACK_FUNCTION: Option<HasSendTrackFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_Transceiver__has_send_track(
    f: HasSendTrackFunction,
) {
    HAS_SEND_TRACK_FUNCTION = Some(f);
}

type SetDirectionFunction = extern "C" fn(Dart_Handle, i32) -> Dart_Handle;
static mut SET_DIRECTION_FUNCTION: Option<SetDirectionFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_Transceiver__set_direction(
    f: SetDirectionFunction,
) {
    SET_DIRECTION_FUNCTION = Some(f);
}

impl Transceiver {
    pub fn current_direction(&self) -> TransceiverDirection {
        unsafe {
            CURRENT_DIRECTION_FUNCTION.unwrap()(self.transceiver.get()).into()
        }
    }

    fn set_direction(
        &self,
        direction: TransceiverDirection,
    ) -> LocalBoxFuture<'static, ()> {
        log::error!("Start set Transceiver::set_direction");
        let fut = VoidDartFuture::new(unsafe {
            SET_DIRECTION_FUNCTION.unwrap()(
                self.transceiver.get(),
                direction.into(),
            )
        });
        Box::pin(async move {
            fut.await;
        })
    }

    pub fn sub_direction(
        &self,
        disabled_direction: TransceiverDirection,
    ) -> LocalBoxFuture<'static, ()> {
        self.set_direction(
            (self.current_direction() - disabled_direction).into(),
        )
    }

    pub fn add_direction(
        &self,
        enabled_direction: TransceiverDirection,
    ) -> LocalBoxFuture<'static, ()> {
        self.set_direction(
            (self.current_direction() | enabled_direction).into(),
        )
    }

    pub fn has_direction(&self, direction: TransceiverDirection) -> bool {
        self.current_direction().contains(direction)
    }

    // TODO: future
    pub async fn set_send_track(
        &self,
        new_sender: Rc<local::Track>,
    ) -> Result<(), platform::Error> {
        // TODO: check this
        unsafe {
            if let Some(sender) =
                GET_SEND_TRACK_FUNCTION.unwrap()(self.transceiver.get()).into()
            {
                REPLACE_TRACK_FUNCTION.unwrap()(
                    sender,
                    new_sender.platform_track().track(),
                );
            }
        }
        // TODO: Replace local::Track of this Transceiver with provided
        // local::Track.
        Ok(())
    }

    pub fn drop_send_track(&self) -> impl Future<Output = ()> {
        // TODO: check this
        unsafe {
            if let Some(sender) =
                GET_SEND_TRACK_FUNCTION.unwrap()(self.transceiver.get()).into()
            {
                DROP_SENDER_FUNCTION.unwrap()(sender);
            }
        }
        async {}
    }

    pub fn set_send_track_enabled(&self, enabled: bool) {
        // TODO: check this
        log::debug!("set_send_track_enabled");
        unsafe {
            if let Some(sender) =
                GET_SEND_TRACK_FUNCTION.unwrap()(self.transceiver.get()).into()
            {
                SET_SEND_TRACK_ENABLED_FUNCTION.unwrap()(sender, enabled);
            }
        }
    }

    pub fn is_stopped(&self) -> bool {
        unsafe { IS_STOPPED_FUNCTION.unwrap()(self.transceiver.get()) }
    }

    pub fn mid(&self) -> Option<String> {
        unsafe { MID_FUNCTION.unwrap()(self.transceiver.get()).into() }
    }

    pub fn send_track(&self) -> Option<Rc<local::Track>> {
        // TODO: check this
        let handle: Dart_Handle = unsafe {
            Option::from(SEND_TRACK_FUNCTION.unwrap()(self.transceiver.get()))
        }?;
        Some(Rc::new(local::Track::new(
            MediaStreamTrack::from(handle),
            MediaSourceKind::Device,
        )))
    }

    pub fn has_send_track(&self) -> bool {
        // TODO: check this
        unsafe { HAS_SEND_TRACK_FUNCTION.unwrap()(self.transceiver.get()) == 1 }
    }
}

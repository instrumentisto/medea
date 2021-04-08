pub mod audio_track_constraints;
mod callback;
mod completer;
pub mod connection_handle;
pub mod device_video_track_constraints;
pub mod display_video_track_constraints;
mod executor;
pub mod input_device_info;
pub mod jason;
pub mod local_media_track;
pub mod media_manager;
pub mod media_stream_settings;
pub mod reconnect_handle;
pub mod remote_media_track;
pub mod room_close_reason;
pub mod room_handle;
mod utils;

use std::{any::Any, marker::PhantomData, time::Duration};

use dart_sys::{Dart_Handle, Dart_PersistentHandle, _Dart_Handle};
use extern_executor::spawn;
use futures_timer::Delay;

use crate::{
    callback::{set_any_closure_caller, AnyClosureCaller, DartCallback},
    completer::Completer,
    connection_handle::ConnectionHandle,
    local_media_track::LocalMediaTrack,
    reconnect_handle::ReconnectHandle,
    room_close_reason::RoomCloseReason,
    room_handle::RoomHandle,
    utils::into_dart_string,
};

struct DartResult(Dart_Handle);

impl<T, E> From<Result<T, E>> for DartResult {
    fn from(_: Result<T, E>) -> Self {
        Self(unsafe {
            new_error_without_source_caller.unwrap()(
                into_dart_string("name".to_string()),
                into_dart_string("message".to_string()),
                into_dart_string("stacktrace".to_string()),
            )
        })
    }
}

impl Into<Dart_Handle> for DartResult {
    fn into(self) -> Dart_Handle {
        self.0
    }
}

static mut new_error_without_source_caller: Option<
    NewErrorWithoutSourceCaller,
> = None;
type NewErrorWithoutSourceCaller = extern "C" fn(
    name: *const libc::c_char,
    message: *const libc::c_char,
    stacktrace: *const libc::c_char,
) -> Dart_Handle;

#[no_mangle]
pub unsafe extern "C" fn register_new_error_without_source_caller(
    c: NewErrorWithoutSourceCaller,
) {
    new_error_without_source_caller = Some(c);
}

static mut new_error_with_source_caller: Option<NewErrorWithSourceCaller> =
    None;
type NewErrorWithSourceCaller = extern "C" fn(
    name: *const libc::c_char,
    message: *const libc::c_char,
    stacktrace: *const libc::c_char,
    source: Dart_Handle,
) -> Dart_Handle;

#[no_mangle]
pub unsafe extern "C" fn register_new_error_with_source_caller(
    c: NewErrorWithSourceCaller,
) {
    new_error_with_source_caller = Some(c);
}

static mut new_ok_caller: Option<NewOkCaller> = None;
type NewOkCaller = extern "C" fn() -> Dart_Handle;

#[no_mangle]
pub unsafe extern "C" fn register_new_ok_caller(c: NewOkCaller) {
    new_ok_caller = Some(c);
}

#[no_mangle]
pub unsafe extern "C" fn test_ok_result() -> Dart_Handle {
    DartResult::from(Ok::<(), ()>(())).into()
}

#[no_mangle]
pub unsafe extern "C" fn test_err_result() -> Dart_Handle {
    DartResult::from(Err::<(), ()>(())).into()
}

#[link(name = "trampoline")]
extern "C" {
    fn Dart_InitializeApiDL(obj: *mut libc::c_void) -> libc::intptr_t;
    fn Dart_NewPersistentHandle_DL_Trampolined(
        object: Dart_Handle,
    ) -> Dart_PersistentHandle;
    fn Dart_HandleFromPersistent_DL_Trampolined(
        object: Dart_PersistentHandle,
    ) -> Dart_Handle;
    fn Dart_DeletePersistentHandle_DL_Trampolined(
        object: Dart_PersistentHandle,
    );
    fn Dart_NewApiError_DL_Trampolined(msg: *const libc::c_char)
        -> Dart_Handle;
    fn Dart_NewUnhandledExceptionError_DL_Trampolined(
        exception: Dart_Handle,
    ) -> Dart_Handle;
    fn Dart_PropagateError_DL_Trampolined(handle: Dart_Handle);
}

#[no_mangle]
pub unsafe extern "C" fn register_any_closure_caller(
    callback: AnyClosureCaller,
) {
    set_any_closure_caller(callback);
}

#[no_mangle]
pub unsafe extern "C" fn test_future() -> Dart_Handle {
    let completer: Completer<(), ()> = Completer::new();
    let future = completer.future();
    spawn(async move {
        Delay::new(Duration::from_millis(3000 as u64)).await;
        completer.complete(());
    });

    future
}

#[no_mangle]
pub unsafe extern "C" fn cb_test(cb: Dart_Handle) {
    DartCallback::<ConnectionHandle>::new(cb).call(ConnectionHandle);
}

#[no_mangle]
pub unsafe extern "C" fn InitDartApiDL(
    obj: *mut libc::c_void,
) -> libc::intptr_t {
    return Dart_InitializeApiDL(obj);
}

#[no_mangle]
pub extern "C" fn dummy_function() {}

pub enum MediaKind {
    Audio = 0,
    Video = 1,
}

impl From<i32> for MediaKind {
    fn from(i: i32) -> Self {
        match i {
            0 => Self::Audio,
            1 => Self::Video,
            _ => unreachable!(),
        }
    }
}

pub enum MediaSourceKind {
    Device = 0,
    Display = 1,
}

impl From<i32> for MediaSourceKind {
    fn from(i: i32) -> Self {
        match i {
            0 => Self::Device,
            1 => Self::Display,
            _ => unreachable!(),
        }
    }
}

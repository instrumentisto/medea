pub mod audio_track_constraints;
pub mod connection_handle;
pub mod device_video_track_constraints;
pub mod display_video_track_constraints;
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

use std::marker::PhantomData;

use dart_sys::{Dart_Handle, Dart_PersistentHandle};

use crate::{
    connection_handle::ConnectionHandle, local_media_track::LocalMediaTrack,
    reconnect_handle::ReconnectHandle, room_close_reason::RoomCloseReason,
    room_handle::RoomHandle,
};

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

static mut VOID_CLOSURE_CALLER: Option<extern "C" fn(c: Dart_Handle)> = None;

#[no_mangle]
pub unsafe extern "C" fn register_closure_caller(
    callback_: extern "C" fn(c: Dart_Handle),
) {
    VOID_CLOSURE_CALLER = Some(callback_);
}

type DartCallbackFP<T> = extern "C" fn(c: Dart_Handle, var: T);

static mut CONNECTION_HANDLE_CLOSURE_CALLER: Option<
    DartCallbackFP<*mut ConnectionHandle>,
> = None;

#[no_mangle]
pub unsafe extern "C" fn register_connection_handle_closure_caller(
    callback: DartCallbackFP<*mut ConnectionHandle>,
) {
    CONNECTION_HANDLE_CLOSURE_CALLER = Some(callback);
}

static mut ROOM_CLOSE_REASON_CLOSURE_CALLER: Option<
    DartCallbackFP<*mut RoomCloseReason>,
> = None;

#[no_mangle]
pub unsafe extern "C" fn register_room_close_reason_closure_caller(
    callback: DartCallbackFP<*mut RoomCloseReason>,
) {
    ROOM_CLOSE_REASON_CLOSURE_CALLER = Some(callback);
}

static mut RECONNECT_HANDLE_CLOSURE_CALLER: Option<
    DartCallbackFP<*mut ReconnectHandle>,
> = None;

#[no_mangle]
pub unsafe extern "C" fn register_reconnect_handle_closure_caller(
    callback: DartCallbackFP<*mut ReconnectHandle>,
) {
    RECONNECT_HANDLE_CLOSURE_CALLER = Some(callback);
}

static mut LOCAL_MEDIA_TRACK_CLOSURE_CALLER: Option<
    DartCallbackFP<*mut LocalMediaTrack>,
> = None;

#[no_mangle]
pub unsafe extern "C" fn register_local_media_track_closure_caller(
    callback: DartCallbackFP<*mut LocalMediaTrack>,
) {
    LOCAL_MEDIA_TRACK_CLOSURE_CALLER = Some(callback);
}

pub struct DartCallback<T> {
    cb: Dart_PersistentHandle,
    _argument_type: PhantomData<T>,
}

unsafe impl<T> Send for DartCallback<T> {}

impl<T> DartCallback<T> {
    pub fn new(cb: Dart_Handle) -> Self {
        Self {
            cb: unsafe { Dart_NewPersistentHandle_DL_Trampolined(cb) },
            _argument_type: PhantomData::default(),
        }
    }
}

impl DartCallback<ConnectionHandle> {
    pub unsafe fn call(&self, arg: ConnectionHandle) {
        let closure_handle = Dart_HandleFromPersistent_DL_Trampolined(self.cb);
        CONNECTION_HANDLE_CLOSURE_CALLER.unwrap()(
            closure_handle,
            Box::into_raw(Box::new(arg)),
        );
    }
}

impl DartCallback<ReconnectHandle> {
    pub unsafe fn call(&self, arg: ReconnectHandle) {
        let closure_handle = Dart_HandleFromPersistent_DL_Trampolined(self.cb);
        RECONNECT_HANDLE_CLOSURE_CALLER.unwrap()(
            closure_handle,
            Box::into_raw(Box::new(arg)),
        );
    }
}

impl DartCallback<RoomCloseReason> {
    pub unsafe fn call(&self, arg: RoomCloseReason) {
        let closure_handle = Dart_HandleFromPersistent_DL_Trampolined(self.cb);
        ROOM_CLOSE_REASON_CLOSURE_CALLER.unwrap()(
            closure_handle,
            Box::into_raw(Box::new(arg)),
        );
    }
}

impl DartCallback<LocalMediaTrack> {
    pub unsafe fn call(&self, arg: LocalMediaTrack) {
        let closure_handle = Dart_HandleFromPersistent_DL_Trampolined(self.cb);
        LOCAL_MEDIA_TRACK_CLOSURE_CALLER.unwrap()(
            closure_handle,
            Box::into_raw(Box::new(arg)),
        );
    }
}

impl DartCallback<()> {
    pub unsafe fn call(&self) {
        let closure_handle = Dart_HandleFromPersistent_DL_Trampolined(self.cb);
        VOID_CLOSURE_CALLER.unwrap()(closure_handle);
    }
}

impl<T> Drop for DartCallback<T> {
    fn drop(&mut self) {
        unsafe { Dart_DeletePersistentHandle_DL_Trampolined(self.cb) };
    }
}

#[no_mangle]
pub unsafe extern "C" fn InitDartApiDL(
    obj: *mut libc::c_void,
) -> libc::intptr_t {
    return Dart_InitializeApiDL(obj);
}

#[no_mangle]
pub unsafe extern "C" fn test_callback(cb: Dart_Handle) {
    let callback = DartCallback::<()>::new(cb);
    callback.call();
}

#[no_mangle]
pub extern "C" fn dummy_function() {}

pub enum MediaKind {
    Audio = 0,
    Video = 1,
}

pub enum MediaSourceKind {
    Device = 0,
    Display = 1,
}

use crate::{
    input_device_info::InputDeviceInfo,
    local_media_track::LocalMediaTrack,
    media_stream_settings::MediaStreamSettings,
    utils::{spawn, Completer, PtrArray},
};
use dart_sys::Dart_Handle;

pub struct MediaManagerHandle;

impl MediaManagerHandle {
    pub async fn enumerate_devices(&self) -> Vec<InputDeviceInfo> {
        // async && Result
        vec![InputDeviceInfo {}, InputDeviceInfo {}, InputDeviceInfo {}]
    }

    pub async fn init_local_tracks(
        &self,
        _caps: &MediaStreamSettings,
    ) -> Vec<LocalMediaTrack> {
        // async && Result
        vec![LocalMediaTrack {}, LocalMediaTrack {}, LocalMediaTrack {}]
    }
}

#[no_mangle]
pub unsafe extern "C" fn MediaManagerHandle__init_local_tracks(
    this: *const MediaManagerHandle,
    caps: *const MediaStreamSettings,
) -> Dart_Handle {
    let this = this.as_ref().unwrap();
    let caps = caps.as_ref().unwrap();
    let completer: Completer<PtrArray<LocalMediaTrack>, ()> = Completer::new();
    let fut = completer.future();
    spawn(async move {
        completer.complete(PtrArray::new(this.init_local_tracks(caps).await));
    });
    fut
}

#[no_mangle]
pub unsafe extern "C" fn MediaManagerHandle__enumerate_devices(
    this: *const MediaManagerHandle,
) -> Dart_Handle {
    let this = this.as_ref().unwrap();
    let completer: Completer<PtrArray<InputDeviceInfo>, ()> = Completer::new();
    let fut = completer.future();
    spawn(async move {
        completer.complete(PtrArray::new(this.enumerate_devices().await));
    });
    fut
}

#[no_mangle]
pub unsafe extern "C" fn MediaManagerHandle__free(
    this: *mut MediaManagerHandle,
) {
    Box::from_raw(this);
}

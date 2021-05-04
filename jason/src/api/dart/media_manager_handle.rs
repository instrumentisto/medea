use dart_sys::Dart_Handle;

use crate::{
    input_device_info::InputDeviceInfo,
    local_media_track::LocalMediaTrack,
    media_stream_settings::MediaStreamSettings,
    utils::{future_to_dart, PtrArray},
    ForeignClass,
};

pub struct MediaManagerHandle;

impl ForeignClass for MediaManagerHandle {}

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

    future_to_dart(async move {
        // TODO: Rust thinks that `this` is static, but its not. We should
        //       always clone everything into `future_to_dart`. This applies to
        //       all external async functions.
        Ok::<_, ()>(PtrArray::new(this.init_local_tracks(caps).await))
    })
}

#[no_mangle]
pub unsafe extern "C" fn MediaManagerHandle__enumerate_devices(
    this: *const MediaManagerHandle,
) -> Dart_Handle {
    let this = this.as_ref().unwrap();

    future_to_dart(async move {
        Ok::<_, ()>(PtrArray::new(this.enumerate_devices().await))
    })
}

#[no_mangle]
pub unsafe extern "C" fn MediaManagerHandle__free(
    this: *mut MediaManagerHandle,
) {
    MediaManagerHandle::from_ptr(this);
}

use std::ptr::NonNull;

use crate::{
    input_device_info::InputDeviceInfo, local_media_track::LocalMediaTrack,
    media_stream_settings::MediaStreamSettings, utils::PtrArray, ForeignClass,
};

pub struct MediaManagerHandle;

impl ForeignClass for MediaManagerHandle {}

impl MediaManagerHandle {
    pub fn enumerate_devices(&self) -> Vec<InputDeviceInfo> {
        // async && Result
        vec![InputDeviceInfo {}, InputDeviceInfo {}, InputDeviceInfo {}]
    }

    pub fn init_local_tracks(
        &self,
        _caps: &MediaStreamSettings,
    ) -> Vec<LocalMediaTrack> {
        // async && Result
        vec![LocalMediaTrack {}, LocalMediaTrack {}, LocalMediaTrack {}]
    }
}

#[no_mangle]
pub unsafe extern "C" fn MediaManagerHandle__init_local_tracks(
    this: NonNull<MediaManagerHandle>,
    caps: NonNull<MediaStreamSettings>,
) -> PtrArray<LocalMediaTrack> {
    let this = this.as_ref();
    let caps = caps.as_ref();

    PtrArray::new(this.init_local_tracks(caps))
}

#[no_mangle]
pub unsafe extern "C" fn MediaManagerHandle__enumerate_devices(
    this: NonNull<MediaManagerHandle>,
) -> PtrArray<InputDeviceInfo> {
    let this = this.as_ref();

    PtrArray::new(this.enumerate_devices())
}

#[no_mangle]
pub unsafe extern "C" fn MediaManagerHandle__free(
    this: NonNull<MediaManagerHandle>,
) {
    MediaManagerHandle::from_ptr(this);
}

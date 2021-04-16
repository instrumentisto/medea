use crate::{
    input_device_info::InputDeviceInfo,
    local_media_track::LocalMediaTrack,
    utils::{ptr_from_dart_as_mut, PtrArray},
};

pub struct MediaManagerHandle;

impl MediaManagerHandle {
    pub fn enumerate_devices(&self) -> Vec<InputDeviceInfo> {
        // async && Result
        vec![InputDeviceInfo {}, InputDeviceInfo {}, InputDeviceInfo {}]
    }

    pub fn init_local_tracks(&self) -> Vec<LocalMediaTrack> {
        // async && Result
        vec![LocalMediaTrack {}, LocalMediaTrack {}, LocalMediaTrack {}]
    }
}

#[no_mangle]
pub unsafe extern "C" fn MediaManagerHandle__init_local_tracks(
    this: *mut MediaManagerHandle,
) -> PtrArray<LocalMediaTrack> {
    PtrArray::new(ptr_from_dart_as_mut(this).init_local_tracks())
}

#[no_mangle]
pub unsafe extern "C" fn MediaManagerHandle__enumerate_devices(
    this: *mut MediaManagerHandle,
) -> PtrArray<InputDeviceInfo> {
    PtrArray::new(ptr_from_dart_as_mut(this).enumerate_devices())
}

#[no_mangle]
pub unsafe extern "C" fn MediaManagerHandle__free(
    this: *mut MediaManagerHandle,
) {
    Box::from_raw(this);
}

use crate::{
    input_device_info::InputDeviceInfo, local_media_track::LocalMediaTrack,
    utils::Array,
};

pub struct MediaManager;

impl MediaManager {
    pub fn enumerate_devices(&self) -> Vec<InputDeviceInfo> {
        todo!()
        // vec![InputDeviceInfo]
    }

    pub fn init_local_tracks(&self) -> Vec<LocalMediaTrack> {
        vec![LocalMediaTrack]
    }
}

#[no_mangle]
pub unsafe extern "C" fn MediaManager__init_local_tracks(
    this: *mut MediaManager,
) -> Array<LocalMediaTrack> {
    let this = Box::from_raw(this);
    Array::new(this.init_local_tracks())
}

#[no_mangle]
pub unsafe extern "C" fn MediaManager__enumerate_devices(this: *mut MediaManager) -> Array<InputDeviceInfo> {
    let this = Box::from_raw(this);
    Array::new(this.enumerate_devices())
}

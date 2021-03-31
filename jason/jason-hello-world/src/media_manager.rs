use std::mem;

use crate::{
    input_device_info::InputDeviceInfo, local_media_track::LocalMediaTrack,
    Array,
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

use crate::{
    audio_track_constraints::AudioTrackConstraints,
    device_video_track_constraints::DeviceVideoTrackConstraints,
    display_video_track_constraints::DisplayVideoTrackConstraints,
    utils::ptr_from_dart_as_mut,
};

pub struct MediaStreamSettings;

impl MediaStreamSettings {
    pub fn new() -> Self {
        Self
    }

    pub fn audio(&mut self, _: AudioTrackConstraints) {}

    pub fn device_video(&mut self, _: DeviceVideoTrackConstraints) {}

    pub fn display_video(&mut self, _: DisplayVideoTrackConstraints) {}
}

#[no_mangle]
pub unsafe extern "C" fn MediaStreamSettings__audio(
    this: *mut MediaStreamSettings,
    constraints: *mut AudioTrackConstraints,
) {
    ptr_from_dart_as_mut(this).audio(*Box::from_raw(constraints));
}

#[no_mangle]
pub unsafe extern "C" fn MediaStreamSettings__device_video(
    this: *mut MediaStreamSettings,
    constraints: *mut DeviceVideoTrackConstraints,
) {
    ptr_from_dart_as_mut(this).device_video(*Box::from_raw(constraints));
}

#[no_mangle]
pub unsafe extern "C" fn MediaStreamSettings__display_video(
    this: *mut MediaStreamSettings,
    constraints: *mut DisplayVideoTrackConstraints,
) {
    ptr_from_dart_as_mut(this).display_video(*Box::from_raw(constraints));
}

#[no_mangle]
pub unsafe extern "C" fn MediaStreamSettings__free(
    this: *mut MediaStreamSettings,
) {
    Box::from_raw(this);
}

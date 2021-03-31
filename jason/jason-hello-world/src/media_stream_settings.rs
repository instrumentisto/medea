use crate::{
    audio_track_constraints::AudioTrackConstraints,
    device_video_track_constraints::DeviceVideoTrackConstraints,
    display_video_track_constraints::DisplayVideoTrackConstraints,
};

pub struct MediaStreamSettings;

impl MediaStreamSettings {
    pub fn audio(&self, constraints: &AudioTrackConstraints) {}

    pub fn device_video(&self, constraints: &DeviceVideoTrackConstraints) {}

    pub fn display_video(&self, constraints: &DisplayVideoTrackConstraints) {}
}

#[no_mangle]
pub unsafe extern "C" fn MediaStreamSettings__audio(
    this: *mut MediaStreamSettings,
    constraints: *mut AudioTrackConstraints,
) {
    let this = Box::from_raw(this);
    let constraints = Box::from_raw(constraints);
    this.audio(&constraints);
}

#[no_mangle]
pub unsafe extern "C" fn MediaStreamSettings__device_video(
    this: *mut MediaStreamSettings,
    constraints: *mut DeviceVideoTrackConstraints,
) {
    let this = Box::from_raw(this);
    let constraints = Box::from_raw(constraints);
    this.device_video(&constraints);
}

#[no_mangle]
pub unsafe extern "C" fn MediaStreamSettings__display_video(
    this: *mut MediaStreamSettings,
    constraints: *mut DisplayVideoTrackConstraints,
) {
    let this = Box::from_raw(this);
    let constraints = Box::from_raw(constraints);
    this.display_video(&constraints);
}

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
pub extern "C" fn MediaStreamSettings__new() -> *const MediaStreamSettings {
    Box::into_raw(Box::new(MediaStreamSettings::new()))
}

#[no_mangle]
pub unsafe extern "C" fn MediaStreamSettings__audio(
    this: *mut MediaStreamSettings,
    constraints: *mut AudioTrackConstraints,
) {
    let this = ptr_from_dart_as_mut(this);

    this.audio(*Box::from_raw(constraints));
}

#[no_mangle]
pub unsafe extern "C" fn MediaStreamSettings__device_video(
    this: *mut MediaStreamSettings,
    constraints: *mut DeviceVideoTrackConstraints,
) {
    let this = ptr_from_dart_as_mut(this);

    this.device_video(*Box::from_raw(constraints));
}

#[no_mangle]
pub unsafe extern "C" fn MediaStreamSettings__display_video(
    this: *mut MediaStreamSettings,
    constraints: *mut DisplayVideoTrackConstraints,
) {
    let this = ptr_from_dart_as_mut(this);

    this.display_video(*Box::from_raw(constraints));
}

#[no_mangle]
pub unsafe extern "C" fn MediaStreamSettings__free(
    this: *mut MediaStreamSettings,
) {
    if !this.is_null() {
        Box::from_raw(this);
    }
}

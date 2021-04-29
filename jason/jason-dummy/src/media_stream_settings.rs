use std::ptr::NonNull;

use crate::{
    audio_track_constraints::AudioTrackConstraints,
    device_video_track_constraints::DeviceVideoTrackConstraints,
    display_video_track_constraints::DisplayVideoTrackConstraints,
    ForeignClass,
};

pub struct MediaStreamSettings;

impl ForeignClass for MediaStreamSettings {}

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
    MediaStreamSettings::new().into_ptr()
}

#[no_mangle]
pub unsafe extern "C" fn MediaStreamSettings__audio(
    mut this: NonNull<MediaStreamSettings>,
    constraints: NonNull<AudioTrackConstraints>,
) {
    let this = this.as_mut();

    this.audio(AudioTrackConstraints::from_ptr(constraints));
}

#[no_mangle]
pub unsafe extern "C" fn MediaStreamSettings__device_video(
    mut this: NonNull<MediaStreamSettings>,
    constraints: NonNull<DeviceVideoTrackConstraints>,
) {
    let this = this.as_mut();

    this.device_video(DeviceVideoTrackConstraints::from_ptr(constraints));
}

#[no_mangle]
pub unsafe extern "C" fn MediaStreamSettings__display_video(
    mut this: NonNull<MediaStreamSettings>,
    constraints: NonNull<DisplayVideoTrackConstraints>,
) {
    let this = this.as_mut();

    this.display_video(DisplayVideoTrackConstraints::from_ptr(constraints));
}

#[no_mangle]
pub unsafe extern "C" fn MediaStreamSettings__free(
    this: NonNull<MediaStreamSettings>,
) {
    MediaStreamSettings::from_ptr(this);
}

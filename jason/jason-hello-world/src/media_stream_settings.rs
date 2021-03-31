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

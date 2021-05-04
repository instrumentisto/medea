use super::{
    audio_track_constraints::AudioTrackConstraints,
    device_video_track_constraints::DeviceVideoTrackConstraints,
    display_video_track_constraints::DisplayVideoTrackConstraints,
    ForeignClass,
};

pub use crate::media::MediaStreamSettings;

impl ForeignClass for MediaStreamSettings {}

/// Creates new [`MediaStreamSettings`] with none constraints configured.
#[no_mangle]
pub extern "C" fn MediaStreamSettings__new() -> *const MediaStreamSettings {
    MediaStreamSettings::new().into_ptr()
}

/// Specifies a nature and settings of an audio [`MediaStreamTrack`].
///
/// [`MediaStreamTrack`]: crate::platform::MediaStreamTrack
#[no_mangle]
pub unsafe extern "C" fn MediaStreamSettings__audio(
    this: *mut MediaStreamSettings,
    constraints: *mut AudioTrackConstraints,
) {
    let this = this.as_mut().unwrap();

    this.audio(AudioTrackConstraints::from_ptr(constraints));
}

/// Set constraints for obtaining a local video sourced from a media device.
#[no_mangle]
pub unsafe extern "C" fn MediaStreamSettings__device_video(
    this: *mut MediaStreamSettings,
    constraints: *mut DeviceVideoTrackConstraints,
) {
    let this = this.as_mut().unwrap();

    this.device_video(DeviceVideoTrackConstraints::from_ptr(constraints));
}

/// Set constraints for capturing a local video from user's display.
#[no_mangle]
pub unsafe extern "C" fn MediaStreamSettings__display_video(
    this: *mut MediaStreamSettings,
    constraints: *mut DisplayVideoTrackConstraints,
) {
    let this = this.as_mut().unwrap();

    this.display_video(DisplayVideoTrackConstraints::from_ptr(constraints));
}

/// Frees the data behind the provided pointer.
///
/// # Safety
///
/// Should be called when object is no longer needed. Calling this more than
/// once for the same pointer is equivalent to double free.
#[no_mangle]
pub unsafe extern "C" fn MediaStreamSettings__free(
    this: *mut MediaStreamSettings,
) {
    let _ = MediaStreamSettings::from_ptr(this);
}

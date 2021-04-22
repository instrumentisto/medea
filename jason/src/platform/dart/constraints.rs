use crate::media::constraints::ConstrainU32;
use dart_sys::{Dart_GetMainPortId, Dart_Handle, _Dart_Handle};
use derive_more::{AsRef, From};

use crate::{
    media::{
        constraints::ConstrainString, AudioTrackConstraints,
        DeviceVideoTrackConstraints, DisplayVideoTrackConstraints,
    },
    platform::dart::utils::{handle::DartHandle, map::DartMap},
};

pub struct MediaTrackConstraints(DartMap);

impl Into<Dart_Handle> for MediaTrackConstraints {
    fn into(self) -> Dart_Handle {
        self.0.into()
    }
}

type NewFunction = extern "C" fn() -> Dart_Handle;
static mut NEW_FUNCTION: Option<NewFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_MediaStreamConstraints__new(f: NewFunction) {
    NEW_FUNCTION = Some(f);
}

type AudioFunction = extern "C" fn(Dart_Handle, Dart_Handle);
static mut AUDIO_FUNCTION: Option<AudioFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_MediaStreamConstraints__audio(
    f: AudioFunction,
) {
    AUDIO_FUNCTION = Some(f);
}

type VideoFunction = extern "C" fn(Dart_Handle, Dart_Handle);
static mut VIDEO_FUNCTION: Option<VideoFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_MediaStreamConstraints__video(
    f: VideoFunction,
) {
    VIDEO_FUNCTION = Some(f);
}

#[derive(Clone, Debug, From)]
pub struct MediaStreamConstraints(DartHandle);

impl Into<Dart_Handle> for MediaStreamConstraints {
    fn into(self) -> Dart_Handle {
        self.0.get()
    }
}

impl MediaStreamConstraints {
    pub fn new() -> Self {
        unsafe { Self(DartHandle::new(NEW_FUNCTION.unwrap()())) }
    }

    pub fn audio(&mut self, audio: AudioTrackConstraints) {
        unsafe {
            AUDIO_FUNCTION.unwrap()(
                self.0.get(),
                MediaTrackConstraints::from(audio).into(),
            );
        }
    }

    pub fn video(&mut self, video: DeviceVideoTrackConstraints) {
        unsafe {
            VIDEO_FUNCTION.unwrap()(
                self.0.get(),
                MediaTrackConstraints::from(video).into(),
            );
        }
    }
}

impl Default for MediaStreamConstraints {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, From)]
pub struct DisplayMediaStreamConstraints(DartHandle);

impl Into<Dart_Handle> for DisplayMediaStreamConstraints {
    fn into(self) -> Dart_Handle {
        self.0.get()
    }
}

impl Default for DisplayMediaStreamConstraints {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl DisplayMediaStreamConstraints {
    /// Creates a new [`DisplayMediaStreamConstraints`] with none constraints
    /// configured.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        unsafe { Self(DartHandle::new(NEW_FUNCTION.unwrap()())) }
    }

    /// Specifies the nature and settings of the `video` [MediaStreamTrack][1].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    #[inline]
    pub fn video(&mut self, video: DisplayVideoTrackConstraints) {
        unsafe {
            VIDEO_FUNCTION.unwrap()(
                self.0.get(),
                MediaTrackConstraints::from(video).into(),
            );
        }
    }
}

impl From<DisplayVideoTrackConstraints> for MediaTrackConstraints {
    fn from(_: DisplayVideoTrackConstraints) -> Self {
        MediaTrackConstraints(DartMap::new())
    }
}

impl From<AudioTrackConstraints> for MediaTrackConstraints {
    fn from(from: AudioTrackConstraints) -> Self {
        let cons = DartMap::new();
        let audio_cons = DartMap::new();
        let ideal_cons = DartMap::new();
        let exact_cons = DartMap::new();
        if let Some(device_id) = from.device_id {
            match device_id {
                ConstrainString::Exact(device_id) => {
                    exact_cons.set("device_id".to_string(), device_id.into());
                }
                ConstrainString::Ideal(device_id) => {
                    ideal_cons.set("device_id".to_string(), device_id.into());
                }
            }
        }
        audio_cons.set("mandatory".to_string(), exact_cons.into());
        audio_cons.set("optional".to_string(), ideal_cons.into());
        cons.set("audio".to_string(), audio_cons.into());
        MediaTrackConstraints(cons)
    }
}

impl From<DeviceVideoTrackConstraints> for MediaTrackConstraints {
    fn from(from: DeviceVideoTrackConstraints) -> Self {
        let video_cons = DartMap::new();
        let ideal_cons = DartMap::new();
        let exact_cons = DartMap::new();
        if let Some(device_id) = from.device_id {
            match device_id {
                ConstrainString::Exact(device_id) => {
                    ideal_cons.set("device_id".to_string(), device_id.into());
                }
                ConstrainString::Ideal(device_id) => {
                    exact_cons.set("device_id".to_string(), device_id.into());
                }
            }
        }
        if let Some(height) = from.height {
            match height {
                ConstrainU32::Ideal(height) => {
                    ideal_cons
                        .set("height".to_string(), (height as i32).into());
                }
                ConstrainU32::Exact(height) => {
                    exact_cons
                        .set("height".to_string(), (height as i32).into());
                }
                ConstrainU32::Range(min, max) => {
                    exact_cons
                        .set("minHeight".to_string(), (min as i32).into());
                    exact_cons
                        .set("maxHeight".to_string(), (max as i32).into());
                }
            }
        }
        if let Some(width) = from.width {
            match width {
                ConstrainU32::Ideal(width) => {
                    ideal_cons.set("width".to_string(), (width as i32).into());
                }
                ConstrainU32::Exact(width) => {
                    exact_cons.set("width".to_string(), (width as i32).into());
                }
                ConstrainU32::Range(min, max) => {
                    exact_cons.set("minWidth".to_string(), (min as i32).into());
                    exact_cons.set("maxWidth".to_string(), (max as i32).into());
                }
            }
        }
        if let Some(facing_mode) = from.facing_mode {
            match facing_mode {
                ConstrainString::Exact(facing_mode) => {
                    video_cons.set(
                        "facing_mode".to_string(),
                        facing_mode.to_string().into(),
                    );
                }
                ConstrainString::Ideal(facing_mode) => {
                    video_cons.set(
                        "facing_mode".to_string(),
                        facing_mode.to_string().into(),
                    );
                }
            }
        }
        video_cons.set("mandatory".to_string(), exact_cons.into());
        video_cons.set("optional".to_string(), ideal_cons.into());
        let cons = DartMap::new();
        cons.set("video".to_string(), video_cons.into());

        MediaTrackConstraints(cons)
    }
}

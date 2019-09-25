use js_sys::Reflect;

use wasm_bindgen::{prelude::*, JsValue};

use web_sys::{
    ConstrainDomStringParameters,
    MediaStreamConstraints as SysMediaStreamConstraints,
    MediaStreamTrack as SysMediaStreamTrack,
    MediaTrackConstraints as SysMediaTrackConstraints,
};

/// [MediaStreamConstraints][1] wrapper.
///
/// [1]: https://www.w3.org/TR/mediacapture-streams/#dom-mediastreamconstraints
#[allow(clippy::module_name_repetitions)]
#[wasm_bindgen]
#[derive(Clone, Default)]
pub struct MediaStreamConstraints {
    audio: Option<AudioTrackConstraints>,
    video: Option<VideoTrackConstraints>,
}

impl MediaStreamConstraints {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn audio(&self) -> &Option<AudioTrackConstraints> {
        &self.audio
    }

    pub fn video(&self) -> &Option<VideoTrackConstraints> {
        &self.video
    }

    pub fn audio_mut(&mut self) -> &mut Option<AudioTrackConstraints> {
        &mut self.audio
    }

    pub fn video_mut(&mut self) -> &mut Option<VideoTrackConstraints> {
        &mut self.video
    }
}

impl From<MediaStreamConstraints> for SysMediaStreamConstraints {
    fn from(constraints: MediaStreamConstraints) -> Self {
        let mut sys_constraints = Self::new();

        if let Some(video) = constraints.video {
            let video: SysMediaTrackConstraints = video.into();
            sys_constraints.video(&video.into());
        }

        if let Some(audio) = constraints.audio {
            let audio: SysMediaTrackConstraints = audio.into();
            sys_constraints.audio(&audio.into());
        }

        sys_constraints
    }
}

// TODO: its gonna be a nightmare if we will add all possible constraints,
//       especially if we will support all that `exact`/`min`/`max`/`ideal`
//       stuff, will need major refactoring then
// TODO: using reflection to get fields values is pure evil, but there are no
//       getters for WebIDL's dictionaries, should be wrapped or improved in
//       wasm-bindgen

/// Represents constraints applicable to audio tracks.
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Default)]
pub struct AudioTrackConstraints {
    /// The identifier of the device generating the content of the media track.
    device_id: Option<String>,
}

impl AudioTrackConstraints {
    pub fn new() -> Self {
        Self::default()
    }

    /// Checks if provided [`MediaStreamTrack`][1] satisfies constraints
    /// contained.
    ///
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack
    pub fn satisfies(&self, track: &SysMediaStreamTrack) -> bool {
        if track.kind() != "audio" {
            return false;
        }

        if let Some(device_id) = &self.device_id {
            let track_device_id = if let Ok(val) = Reflect::get(
                track.get_settings().as_ref(),
                &JsValue::from_str("deviceId"),
            ) {
                if let Some(val) = val.as_string() {
                    val
                } else {
                    return false;
                }
            } else {
                return false;
            };

            if track_device_id.as_str() != device_id {
                return false;
            }
        }

        true
    }

    /// Sets [deviceId][1] constraint.
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#def-constraint-deviceId
    pub fn device_id(&mut self, device_id: String) -> &mut Self {
        self.device_id = Some(device_id);
        self
    }
}

impl Into<SysMediaTrackConstraints> for AudioTrackConstraints {
    fn into(self) -> SysMediaTrackConstraints {
        let mut constraints = SysMediaTrackConstraints::new();

        if let Some(device_id) = self.device_id {
            let mut val = ConstrainDomStringParameters::new();
            val.exact(&(device_id.into()));
            constraints.device_id(&(val.into()));
        }

        constraints
    }
}

/// Represents constraints applicable to video tracks.
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Default)]
pub struct VideoTrackConstraints {
    /// The identifier of the device generating the content of the media track.
    device_id: Option<String>,
}

impl VideoTrackConstraints {
    pub fn new() -> Self {
        Self::default()
    }

    /// Checks if provided [`MediaStreamTrack`][1] satisfies constraints
    /// contained.
    ///
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack
    pub fn satisfies(&self, track: &SysMediaStreamTrack) -> bool {
        if track.kind() != "video" {
            return false;
        }

        if let Some(device_id) = &self.device_id {
            let track_device_id = if let Ok(val) = Reflect::get(
                track.get_settings().as_ref(),
                &JsValue::from_str("deviceId"),
            ) {
                if let Some(val) = val.as_string() {
                    val
                } else {
                    return false;
                }
            } else {
                return false;
            };

            if track_device_id.as_str() != device_id {
                return false;
            }
        }

        true
    }

    /// Sets [deviceId][1] constraint.
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#def-constraint-deviceId
    pub fn device_id(&mut self, device_id: String) -> &mut Self {
        self.device_id = Some(device_id);
        self
    }
}

impl Into<SysMediaTrackConstraints> for VideoTrackConstraints {
    fn into(self) -> SysMediaTrackConstraints {
        let mut constraints = SysMediaTrackConstraints::new();

        if let Some(device_id) = self.device_id {
            let mut val = ConstrainDomStringParameters::new();
            val.exact(&(device_id.into()));
            constraints.device_id(&(val.into()));
        }

        constraints
    }
}

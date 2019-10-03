use wasm_bindgen::prelude::*;
use web_sys::{
    ConstrainDomStringParameters,
    MediaStreamConstraints as SysMediaStreamConstraints,
    MediaStreamTrack as SysMediaStreamTrack,
    MediaTrackConstraints as SysMediaTrackConstraints,
};

use crate::utils::get_property_by_name;

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

#[wasm_bindgen]
impl MediaStreamConstraints {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Specifies the nature and settings of the audio [MediaStreamTrack].
    pub fn audio(&mut self, constraints: AudioTrackConstraints) {
        self.audio.replace(constraints);
    }

    /// Specifies the nature and settings of the video [MediaStreamTrack].
    pub fn video(&mut self, constraints: VideoTrackConstraints) {
        self.video.replace(constraints);
    }
}

impl MediaStreamConstraints {
    pub fn get_audio(&self) -> &Option<AudioTrackConstraints> {
        &self.audio
    }

    pub fn get_video(&self) -> &Option<VideoTrackConstraints> {
        &self.video
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

/// Checks that the [MediaStreamTrack] is taken from a device
/// with given [deviceId][1].
///
/// [1]: https://www.w3.org/TR/mediacapture-streams/#def-constraint-deviceId
macro_rules! satisfies_by_device_id {
    ($v:expr, $track:ident) => {{
        match &$v.device_id {
            None => true,
            Some(device_id) => get_property_by_name(
                &$track.get_settings(),
                "deviceId",
                |val| val.as_string(),
            )
            .map_or(false, |id| id.as_str() == device_id),
        }
    }};
}

// TODO: its gonna be a nightmare if we will add all possible constraints,
//       especially if we will support all that `exact`/`min`/`max`/`ideal`
//       stuff, will need major refactoring then
// TODO: using reflection to get fields values is pure evil, but there are no
//       getters for WebIDL's dictionaries, should be wrapped or improved in
//       wasm-bindgen

/// Represents constraints applicable to audio tracks.
#[allow(clippy::module_name_repetitions)]
#[wasm_bindgen]
#[derive(Clone, Default)]
pub struct AudioTrackConstraints {
    /// The identifier of the device generating the content of the media track.
    device_id: Option<String>,
}

#[wasm_bindgen]
impl AudioTrackConstraints {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets [deviceId][1] constraint.
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#def-constraint-deviceId
    pub fn device_id(&mut self, device_id: String) {
        self.device_id = Some(device_id);
    }
}

impl AudioTrackConstraints {
    /// Checks if provided [`MediaStreamTrack`][1] satisfies constraints
    /// contained.
    ///
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack
    pub fn satisfies(&self, track: &SysMediaStreamTrack) -> bool {
        if track.kind() != "audio" {
            return false;
        }

        satisfies_by_device_id!(self, track)
    }
}

impl From<AudioTrackConstraints> for SysMediaTrackConstraints {
    fn from(track_constraints: AudioTrackConstraints) -> Self {
        let mut constraints = Self::new();

        if let Some(device_id) = track_constraints.device_id {
            let mut val = ConstrainDomStringParameters::new();
            val.exact(&(device_id.into()));
            constraints.device_id(&(val.into()));
        }

        constraints
    }
}

/// Represents constraints applicable to video tracks.
#[allow(clippy::module_name_repetitions)]
#[wasm_bindgen]
#[derive(Clone, Default)]
pub struct VideoTrackConstraints {
    /// The identifier of the device generating the content of the media track.
    device_id: Option<String>,
}

#[wasm_bindgen]
impl VideoTrackConstraints {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets [deviceId][1] constraint.
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#def-constraint-deviceId
    pub fn device_id(&mut self, device_id: String) {
        self.device_id = Some(device_id);
    }
}

impl VideoTrackConstraints {
    /// Checks if provided [`MediaStreamTrack`][1] satisfies constraints
    /// contained.
    ///
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack
    pub fn satisfies(&self, track: &SysMediaStreamTrack) -> bool {
        if track.kind() != "video" {
            return false;
        }

        satisfies_by_device_id!(self, track)
    }
}

impl From<VideoTrackConstraints> for SysMediaTrackConstraints {
    fn from(track_constraints: VideoTrackConstraints) -> Self {
        let mut constraints = Self::new();

        if let Some(device_id) = track_constraints.device_id {
            let mut val = ConstrainDomStringParameters::new();
            val.exact(&(device_id.into()));
            constraints.device_id(&(val.into()));
        }

        constraints
    }
}

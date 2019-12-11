use medea_client_api_proto::{
    AudioSettings as ProtoAudioConstraints, MediaType as ProtoTrackConstraints,
    VideoSettings as ProtoVideoConstraints,
};
use wasm_bindgen::prelude::*;
use web_sys::{
    ConstrainDomStringParameters,
    MediaStreamConstraints as SysMediaStreamConstraints,
    MediaStreamTrack as SysMediaStreamTrack, MediaStreamTrackState,
    MediaTrackConstraints as SysMediaTrackConstraints,
};

use crate::utils::get_property_by_name;

/// [MediaStreamConstraints][1] wrapper.
///
/// [1]: https://www.w3.org/TR/mediacapture-streams/#dom-mediastreamconstraints
#[wasm_bindgen]
#[derive(Clone, Default)]
pub struct MediaStreamConstraints {
    audio: Option<AudioTrackConstraints>,
    video: Option<VideoTrackConstraints>,
}

#[wasm_bindgen]
impl MediaStreamConstraints {
    /// Creates new [`MediaStreamConstraints`] with none constraints configured.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Specifies the nature and settings of the audio [MediaStreamTrack][1].
    ///
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack
    pub fn audio(&mut self, constraints: AudioTrackConstraints) {
        self.audio.replace(constraints);
    }

    /// Specifies the nature and settings of the video [MediaStreamTrack][1].
    ///
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack
    pub fn video(&mut self, constraints: VideoTrackConstraints) {
        self.video.replace(constraints);
    }
}

impl MediaStreamConstraints {
    /// Returns only audio constraints.
    pub fn get_audio(&self) -> &Option<AudioTrackConstraints> {
        &self.audio
    }

    /// Returns only video constraints.
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

/// Checks that the [MediaStreamTrack][1] is taken from a device
/// with given [deviceId][2].
///
/// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack
/// [2]: https://www.w3.org/TR/mediacapture-streams/#def-constraint-deviceId
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

/// Wrapper around [MediaTrackConstraints][1].
///
/// [1]: https://w3.org/TR/mediacapture-streams/#media-track-constraints
#[derive(Clone)]
pub enum TrackConstraints {
    /// Audio constraints.
    Audio(AudioTrackConstraints),
    /// Video constraints.
    Video(VideoTrackConstraints),
}

impl TrackConstraints {
    /// Checks if provided [MediaStreamTrack][1] satisfies this
    /// [`TrackConstraints`].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    pub fn satisfies(&self, track: &SysMediaStreamTrack) -> bool {
        match self {
            Self::Audio(audio) => audio.satisfies(&track),
            Self::Video(video) => video.satisfies(&track),
        }
    }
}

impl From<ProtoTrackConstraints> for TrackConstraints {
    fn from(caps: ProtoTrackConstraints) -> Self {
        match caps {
            ProtoTrackConstraints::Audio(audio) => Self::Audio(audio.into()),
            ProtoTrackConstraints::Video(video) => Self::Video(video.into()),
        }
    }
}

// TODO: Its gonna be a nightmare if we will add all possible constraints,
//       especially if we will support all that `exact`/`min`/`max`/`ideal`
//       stuff, will need major refactoring then.
// TODO: Using reflection to get fields values is pure evil, but there are no
//       getters for WebIDL's dictionaries, should be wrapped or improved in
//       wasm-bindgen.

/// Constraints applicable to audio tracks.
#[wasm_bindgen]
#[derive(Clone, Default)]
pub struct AudioTrackConstraints {
    /// The identifier of the device generating the content for the media
    /// track.
    device_id: Option<String>,
}

#[wasm_bindgen]
impl AudioTrackConstraints {
    /// Creates new [`AudioTrackConstraints`] with none constraints configured.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets [deviceId][1] constraint.
    ///
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#def-constraint-deviceId
    pub fn device_id(&mut self, device_id: String) {
        self.device_id = Some(device_id);
    }
}

impl AudioTrackConstraints {
    /// Checks if provided [MediaStreamTrack][1] satisfies constraints
    /// contained.
    ///
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack
    pub fn satisfies(&self, track: &SysMediaStreamTrack) -> bool {
        if track.kind() != "audio" {
            return false;
        }

        if track.ready_state() != MediaStreamTrackState::Live {
            return false;
        }

        satisfies_by_device_id!(self, track)
        // TODO returns Result<bool, Error>
    }
}

impl From<ProtoAudioConstraints> for AudioTrackConstraints {
    #[inline]
    fn from(_: ProtoAudioConstraints) -> Self {
        Self::new()
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

/// Constraints applicable to video tracks.
#[wasm_bindgen]
#[derive(Clone, Default)]
pub struct VideoTrackConstraints {
    /// The identifier of the device generating the content for the media
    /// track.
    device_id: Option<String>,
}

#[wasm_bindgen]
impl VideoTrackConstraints {
    /// Creates new [`VideoTrackConstraints`] with none constraints configured.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets [deviceId][1] constraint.
    ///
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#def-constraint-deviceId
    pub fn device_id(&mut self, device_id: String) {
        self.device_id = Some(device_id);
    }
}

impl VideoTrackConstraints {
    /// Checks if provided [MediaStreamTrack][1] satisfies constraints
    /// contained.
    ///
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack
    pub fn satisfies(&self, track: &SysMediaStreamTrack) -> bool {
        if track.kind() != "video" {
            return false;
        }

        if track.ready_state() != MediaStreamTrackState::Live {
            return false;
        }

        satisfies_by_device_id!(self, track)
    }
}

impl From<ProtoVideoConstraints> for VideoTrackConstraints {
    fn from(_caps: ProtoVideoConstraints) -> Self {
        Self::new()
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

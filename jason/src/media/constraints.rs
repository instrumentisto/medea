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

/// Helper to distinguish objects related to media captured from device and
/// media captured from display.
#[derive(Clone)]
enum StreamSource<D, S> {
    Device(D),
    Display(S),
}

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

    /// Set constraints that will be used to obtain local video sourced from
    /// media device.
    pub fn device_video(&mut self, constraints: DeviceVideoTrackConstraints) {
        self.video.replace(constraints.into());
    }

    /// Set constraints that will be used to capture local video from user
    /// display.
    pub fn display_video(&mut self, constraints: DisplayVideoTrackConstraints) {
        self.video.replace(constraints.into());
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

    /// Set [`VideoTrackConstraints`].
    pub fn video(&mut self, constraints: VideoTrackConstraints) {
        self.video.replace(constraints);
    }
}

// TODO: DisplayMediaStreamConstraints should be used when it will be
//       implemented.

/// Wrapper around [MediaStreamConstraints][1] that specifies concrete media
/// source (device or display), and allows to group two requests with different
/// sources.
///
/// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastreamconstraints
pub enum MultiSourceMediaStreamConstraints {
    /// Only [getUserMedia()][1] request is required.
    ///
    /// [1]: https://tinyurl.com/rnxcavf
    Device(SysMediaStreamConstraints),

    /// Only [getDisplayMedia()][1] request is required.
    ///
    /// [1]: https://tinyurl.com/wotjrns
    Display(SysMediaStreamConstraints),

    /// Both [getUserMedia()][1] and [getDisplayMedia()][2] are required.
    ///
    /// [1]: https://tinyurl.com/rnxcavf
    /// [2]: https://tinyurl.com/wotjrns
    DeviceAndDisplay(SysMediaStreamConstraints, SysMediaStreamConstraints),
}

impl From<MediaStreamConstraints> for MultiSourceMediaStreamConstraints {
    fn from(constraints: MediaStreamConstraints) -> Self {
        use MultiSourceMediaStreamConstraints::*;

        let mut sys_constraints = SysMediaStreamConstraints::new();
        let video = match constraints.video {
            Some(video) => match video.0 {
                Some(StreamSource::Device(device)) => {
                    sys_constraints
                        .video(&SysMediaTrackConstraints::from(device).into());
                    StreamSource::Device(sys_constraints)
                }
                Some(StreamSource::Display(display)) => {
                    sys_constraints
                        .video(&SysMediaTrackConstraints::from(display).into());
                    StreamSource::Display(sys_constraints)
                }
                None => {
                    // defaults to device video
                    sys_constraints
                        .video(&SysMediaTrackConstraints::new().into());
                    StreamSource::Device(sys_constraints)
                }
            },
            None => StreamSource::Device(sys_constraints),
        };

        match (constraints.audio, video) {
            (Some(audio), StreamSource::Device(mut caps)) => {
                caps.audio(&SysMediaTrackConstraints::from(audio).into());
                Device(caps)
            }
            (Some(audio), StreamSource::Display(caps)) => {
                let mut audio_caps = SysMediaStreamConstraints::new();
                audio_caps.audio(&SysMediaTrackConstraints::from(audio).into());

                DeviceAndDisplay(audio_caps, caps)
            }
            (None, StreamSource::Device(caps)) => Device(caps),
            (None, StreamSource::Display(caps)) => Display(caps),
        }
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
#[derive(Clone)]
pub struct VideoTrackConstraints(
    Option<
        StreamSource<DeviceVideoTrackConstraints, DisplayVideoTrackConstraints>,
    >,
);

/// Constraints applicable to video tracks that are sourced from some media
/// device.
#[wasm_bindgen]
#[derive(Clone, Default)]
pub struct DeviceVideoTrackConstraints {
    /// The identifier of the device generating the content for the media
    /// track.
    device_id: Option<String>,
}

/// Constraints applicable to video tracks that are sourced from screen-capture.
#[wasm_bindgen]
impl DeviceVideoTrackConstraints {
    /// Creates new [`DeviceVideoTrackConstraints`] with none constraints
    /// configured.
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

/// Constraints applicable to video tracks sourced from screen capture.
#[wasm_bindgen]
#[derive(Clone, Default)]
pub struct DisplayVideoTrackConstraints {}

#[wasm_bindgen]
impl DisplayVideoTrackConstraints {
    /// Creates new [`DisplayVideoTrackConstraints`] with none constraints
    /// configured.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
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

        match &self.0 {
            None => true,
            Some(StreamSource::Device(constraints)) => {
                satisfies_by_device_id!(constraints, track)
                    && !Self::guess_is_from_display(&track)
            }
            Some(StreamSource::Display(_)) => {
                Self::guess_is_from_display(&track)
            }
        }
    }

    /// Detect is video track captured from display searching [specific
    /// fields][1] in its settings. Only works fo Chrome atm.
    ///
    /// [1]: https://tinyurl.com/ufx7mcw
    fn guess_is_from_display(track: &SysMediaStreamTrack) -> bool {
        let settings = track.get_settings();

        let has_display_surface =
            get_property_by_name(&settings, "displaySurface", |val| {
                val.as_string()
            })
            .is_some();

        if has_display_surface {
            true
        } else {
            get_property_by_name(&settings, "logicalSurface", |val| {
                val.as_string()
            })
            .is_some()
        }
    }
}

impl From<ProtoVideoConstraints> for VideoTrackConstraints {
    fn from(_caps: ProtoVideoConstraints) -> Self {
        Self(None)
    }
}

impl From<DeviceVideoTrackConstraints> for SysMediaTrackConstraints {
    fn from(track_constraints: DeviceVideoTrackConstraints) -> Self {
        let mut constraints = Self::new();

        if let Some(device_id) = track_constraints.device_id {
            let mut val = ConstrainDomStringParameters::new();
            val.exact(&(device_id.into()));
            constraints.device_id(&(val.into()));
        }

        constraints
    }
}

impl From<DisplayVideoTrackConstraints> for SysMediaTrackConstraints {
    fn from(_: DisplayVideoTrackConstraints) -> Self {
        Self::new()
    }
}

impl From<DeviceVideoTrackConstraints> for VideoTrackConstraints {
    fn from(constraints: DeviceVideoTrackConstraints) -> Self {
        Self(Some(StreamSource::Device(constraints)))
    }
}

impl From<DisplayVideoTrackConstraints> for VideoTrackConstraints {
    fn from(constraints: DisplayVideoTrackConstraints) -> Self {
        Self(Some(StreamSource::Display(constraints)))
    }
}

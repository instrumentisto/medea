use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use derive_more::Display;
use medea_client_api_proto::{
    AudioSettings as ProtoAudioConstraints, MediaType as ProtoTrackConstraints,
    MediaType, VideoSettings as ProtoVideoConstraints,
};
use wasm_bindgen::prelude::*;
use web_sys::{
    ConstrainDomStringParameters,
    ConstrainDomStringParameters as SysConstrainDomStringParameters,
    MediaStreamConstraints as SysMediaStreamConstraints,
    MediaStreamTrack as SysMediaStreamTrack, MediaStreamTrackState,
    MediaTrackConstraints as SysMediaTrackConstraints,
};

use crate::{peer::TransceiverKind, utils::get_property_by_name};

/// Local media stream for injecting into new created [`PeerConnection`]s.
#[derive(Clone, Debug, Default)]
pub struct LocalStreamConstraints(Rc<RefCell<MediaStreamSettings>>);

/// Constraints to the media received from remote. Used to disable or enable
/// media receiving.
pub struct RecvConstraints {
    /// Is audio receiving enabled.
    is_audio_enabled: Cell<bool>,

    /// Is video receiving enabled.
    is_video_enabled: Cell<bool>,
}

impl Default for RecvConstraints {
    fn default() -> Self {
        Self {
            is_audio_enabled: Cell::new(true),
            is_video_enabled: Cell::new(true),
        }
    }
}

impl RecvConstraints {
    /// Enables or disables audio or video receiving.
    pub fn set_enabled(&self, enabled: bool, kind: TransceiverKind) {
        match kind {
            TransceiverKind::Audio => {
                self.is_audio_enabled.set(enabled);
            }
            TransceiverKind::Video => {
                self.is_video_enabled.set(enabled);
            }
        }
    }

    /// Returns is audio receiving enabled.
    pub fn is_audio_enabled(&self) -> bool {
        self.is_audio_enabled.get()
    }

    /// Returns is video receiving enabled.
    pub fn is_video_enabled(&self) -> bool {
        self.is_video_enabled.get()
    }
}

#[cfg(feature = "mockable")]
impl From<MediaStreamSettings> for LocalStreamConstraints {
    #[inline]
    fn from(from: MediaStreamSettings) -> Self {
        Self(Rc::new(RefCell::new(from)))
    }
}

impl LocalStreamConstraints {
    /// Returns new [`LocalStreamConstraints`] with default values.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Constrains the underlying [`MediaStreamSettings`] with the given `other`
    /// [`MediaStreamSettings`].
    #[inline]
    pub fn constrain(&self, other: MediaStreamSettings) {
        self.0.borrow_mut().constrain(other)
    }

    /// Clones underlying [`MediaStreamSettings`].
    #[inline]
    pub fn inner(&self) -> MediaStreamSettings {
        self.0.borrow().clone()
    }

    /// Enables or disables audio or video type in underlying
    /// [`MediaStreamSettings`].
    ///
    /// Doesn't do anything if no [`MediaStreamSettings`] was set.
    ///
    /// If some type of the [`MediaStreamSettings`] is disabled, then this kind
    /// of media won't be published.
    #[inline]
    pub fn set_enabled(&self, enabled: bool, kind: TransceiverKind) {
        self.0.borrow_mut().set_track_enabled(enabled, kind);
    }

    /// Indicates whether provided [`MediaType`] is enabled in the underlying
    /// [`MediaStreamSettings`].
    #[inline]
    pub fn is_enabled(&self, kind: &MediaType) -> bool {
        self.0.borrow_mut().is_enabled(kind)
    }
}

/// Helper to distinguish objects related to media captured from device and
/// media captured from display.
#[derive(Clone, Debug)]
enum StreamSource<D, S> {
    Device(D),
    Display(S),
}

impl StreamSource<DeviceVideoTrackConstraints, DisplayVideoTrackConstraints> {
    fn merge(
        &mut self,
        other: StreamSource<
            DeviceVideoTrackConstraints,
            DisplayVideoTrackConstraints,
        >,
    ) {
        use StreamSource::{Device, Display};

        match self {
            Device(this) => {
                if let Device(that) = other {
                    this.merge(that);
                }
            }
            Display(this) => {
                if let Display(that) = other {
                    this.merge(that);
                }
            }
        };
    }
}

/// [MediaStreamConstraints][1] for the audio media type.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamconstraints
#[derive(Clone, Debug)]
struct AudioMediaStreamSettings {
    /// Constraints applicable to video tracks.
    constraints: AudioTrackConstraints,

    /// Indicator whether audio is enabled and this constraints should be
    /// injected into `Peer`.
    is_enabled: bool,
}

impl Default for AudioMediaStreamSettings {
    #[inline]
    fn default() -> Self {
        Self {
            constraints: AudioTrackConstraints::default(),
            is_enabled: true,
        }
    }
}

/// [MediaStreamConstraints][1] for the video media type.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamconstraints
#[derive(Clone, Debug)]
struct VideoMediaStreamSettings {
    /// Constraints applicable to video tracks.
    constraints: VideoTrackConstraints,

    /// Indicator whether video is enabled and this constraints should be
    /// injected into `Peer`.
    is_enabled: bool,
}

impl Default for VideoMediaStreamSettings {
    #[inline]
    fn default() -> Self {
        Self {
            constraints: VideoTrackConstraints::default(),
            is_enabled: true,
        }
    }
}

/// [MediaStreamConstraints][1] wrapper.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamconstraints
#[wasm_bindgen]
#[derive(Clone, Debug, Default)]
pub struct MediaStreamSettings {
    /// [MediaStreamConstraints][1] for the audio media type.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamconstraints
    audio: AudioMediaStreamSettings,

    /// [MediaStreamConstraints][1] for the video media type.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamconstraints
    video: VideoMediaStreamSettings,
}

#[wasm_bindgen]
impl MediaStreamSettings {
    /// Creates new [`MediaStreamConstraints`] with none constraints configured.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            audio: AudioMediaStreamSettings {
                constraints: AudioTrackConstraints::default(),
                is_enabled: false,
            },
            video: VideoMediaStreamSettings {
                constraints: VideoTrackConstraints::default(),
                is_enabled: false,
            },
        }
    }

    /// Specifies the nature and settings of the audio [MediaStreamTrack][1].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    pub fn audio(&mut self, constraints: AudioTrackConstraints) {
        self.audio.is_enabled = true;
        self.audio.constraints = constraints;
    }

    /// Set constraints that will be used to obtain local video sourced from
    /// media device.
    pub fn device_video(&mut self, constraints: DeviceVideoTrackConstraints) {
        self.video.is_enabled = true;
        self.video.constraints = constraints.into();
    }

    /// Set constraints that will be used to capture local video from user
    /// display.
    pub fn display_video(&mut self, constraints: DisplayVideoTrackConstraints) {
        self.video.is_enabled = true;
        self.video.constraints = constraints.into();
    }
}

impl MediaStreamSettings {
    /// Returns only audio constraints.
    #[inline]
    pub fn get_audio(&self) -> &AudioTrackConstraints {
        &self.audio.constraints
    }

    /// Returns only video constraints.
    #[inline]
    pub fn get_video(&self) -> &VideoTrackConstraints {
        &self.video.constraints
    }

    /// Set [`VideoTrackConstraints`].
    #[inline]
    pub fn video(&mut self, constraints: VideoTrackConstraints) {
        self.video.is_enabled = true;
        self.video.constraints = constraints;
    }

    /// Enables or disables audio or video type in this [`MediaStreamSettings`].
    ///
    /// If some type of the [`MediaStreamSettings`] is disabled, then this kind
    /// of media won't be published.
    #[inline]
    pub fn set_track_enabled(&mut self, enabled: bool, kind: TransceiverKind) {
        match kind {
            TransceiverKind::Audio => {
                self.toggle_publish_audio(enabled);
            }
            TransceiverKind::Video => {
                self.toggle_publish_video(enabled);
            }
        }
    }

    /// Sets the underlying [`AudioMediaStreamSettings::is_enabled`] to the
    /// given value.
    #[inline]
    pub fn toggle_publish_audio(&mut self, is_enabled: bool) {
        self.audio.is_enabled = is_enabled;
    }

    /// Sets the underlying [`VideoMediaStreamSettings::is_enabled`] to the
    /// given value.
    #[inline]
    pub fn toggle_publish_video(&mut self, is_enabled: bool) {
        self.video.is_enabled = is_enabled;
    }

    /// Indicates whether audio is enabled in this [`MediaStreamSettings`].
    #[inline]
    pub fn is_audio_enabled(&self) -> bool {
        self.audio.is_enabled
    }

    /// Indicates whether video is enabled in this [`MediaStreamSettings`].
    #[inline]
    pub fn is_video_enabled(&self) -> bool {
        self.video.is_enabled
    }

    /// Indicates whether the given [`MediaType`] is enabled in this
    /// [`MediaStreamSettings`].
    #[inline]
    pub fn is_enabled(&self, kind: &MediaType) -> bool {
        match kind {
            MediaType::Video(_) => self.video.is_enabled,
            MediaType::Audio(_) => self.audio.is_enabled,
        }
    }

    /// Constrains this [`MediaStreamSettings`] with the given `other`
    /// [`MediaStreamSettings`].
    #[inline]
    fn constrain(&mut self, other: Self) {
        // `&=` cause we should not unmute muted Room, but we can mute not muted
        // room.
        self.audio.is_enabled &= other.audio.is_enabled;
        self.video.is_enabled &= other.video.is_enabled;

        self.audio.constraints = other.audio.constraints;
        self.video.constraints = other.video.constraints;
    }
}

// TODO: DisplayMediaStreamConstraints should be used when it will be
//       implemented by UA's.

/// Wrapper around [MediaStreamConstraints][1] that specifies concrete media
/// source (device or display), and allows to group two requests with different
/// sources.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#mediastreamconstraints
pub enum MultiSourceMediaStreamConstraints {
    /// Only [getUserMedia()][1] request is required.
    ///
    /// [1]: https://tinyurl.com/rnxcavf
    Device(SysMediaStreamConstraints),

    /// Only [getDisplayMedia()][1] request is required.
    ///
    /// [1]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
    Display(SysMediaStreamConstraints),

    /// Both [getUserMedia()][1] and [getDisplayMedia()][2] are required.
    ///
    /// [1]: https://tinyurl.com/rnxcavf
    /// [2]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
    DeviceAndDisplay(SysMediaStreamConstraints, SysMediaStreamConstraints),
}

/// TL;DR:
/// `{None, None}` => `None`
/// `{None, Device}` => `Device`
/// `{None, Display}` => `Display`
/// `{None, Any}` => `Device`
/// `{Some, None}` => `Device`
/// `{Some, Device}` => `Device`
/// `{Some, Display}` => `DeviceAndDisplay`
/// `{Some, Any}` => `Device`
impl From<MediaStreamSettings> for Option<MultiSourceMediaStreamConstraints> {
    fn from(constraints: MediaStreamSettings) -> Self {
        use MultiSourceMediaStreamConstraints as C;

        let (audio, video) = (constraints.audio, constraints.video);
        let mut sys_constraints = SysMediaStreamConstraints::new();
        let video = if video.is_enabled {
            match video.constraints.constraints {
                Some(StreamSource::Device(device)) => {
                    sys_constraints
                        .video(&SysMediaTrackConstraints::from(device).into());
                    Some(StreamSource::Device(sys_constraints))
                }
                Some(StreamSource::Display(display)) => {
                    sys_constraints
                        .video(&SysMediaTrackConstraints::from(display).into());
                    Some(StreamSource::Display(sys_constraints))
                }
                None => {
                    // defaults to device video
                    sys_constraints
                        .video(&SysMediaTrackConstraints::new().into());
                    Some(StreamSource::Device(sys_constraints))
                }
            }
        } else {
            None
        };

        if audio.is_enabled {
            match video {
                Some(StreamSource::Device(mut caps)) => {
                    caps.audio(
                        &SysMediaTrackConstraints::from(audio.constraints)
                            .into(),
                    );
                    Some(C::Device(caps))
                }
                Some(StreamSource::Display(caps)) => {
                    let mut audio_caps = SysMediaStreamConstraints::new();
                    audio_caps.audio(
                        &SysMediaTrackConstraints::from(audio.constraints)
                            .into(),
                    );

                    Some(C::DeviceAndDisplay(audio_caps, caps))
                }
                None => {
                    let mut audio_caps = SysMediaStreamConstraints::new();
                    audio_caps.audio(
                        &SysMediaTrackConstraints::from(audio.constraints)
                            .into(),
                    );
                    Some(C::Device(audio_caps))
                }
            }
        } else {
            match video {
                Some(StreamSource::Device(caps)) => Some(C::Device(caps)),
                Some(StreamSource::Display(caps)) => Some(C::Display(caps)),
                None => None,
            }
        }
    }
}

/// Checks that the [MediaStreamTrack][1] is taken from a device
/// with given [deviceId][2].
///
/// [1]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
/// [2]: https://w3.org/TR/mediacapture-streams/#def-constraint-deviceId
fn satisfies_by_device_id(
    device_id: &Option<String>,
    track: &SysMediaStreamTrack,
) -> bool {
    match device_id {
        None => true,
        Some(device_id) => {
            get_property_by_name(&track.get_settings(), "deviceId", |v| {
                v.as_string()
            })
            .map_or(false, |id| id.as_str() == device_id)
        }
    }
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
    pub fn satisfies<T: AsRef<SysMediaStreamTrack>>(&self, track: T) -> bool {
        match self {
            Self::Audio(audio) => audio.satisfies(&track),
            Self::Video(video) => video.satisfies(&track),
        }
    }

    /// Returns importance of this [`TrackConstraints`].
    ///
    /// If this [`TrackConstraints`] is important then without this
    /// [`TrackConstraints`] call session can't be started.
    pub fn is_required(&self) -> bool {
        match self {
            TrackConstraints::Video(video) => video.is_required,
            TrackConstraints::Audio(audio) => audio.is_required,
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
#[derive(Clone, Debug, Default)]
pub struct AudioTrackConstraints {
    /// The identifier of the device generating the content for the media
    /// track.
    device_id: Option<String>,

    /// Importance of this [`AudioTrackConstraints`].
    ///
    /// If `true` then without this [`AudioTrackConstraints`] call session
    /// can't be started.
    is_required: bool,
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
    /// [1]: https://w3.org/TR/mediacapture-streams/#def-constraint-deviceId
    pub fn device_id(&mut self, device_id: String) {
        self.device_id = Some(device_id);
    }
}

impl AudioTrackConstraints {
    /// Checks if provided [MediaStreamTrack][1] satisfies constraints
    /// contained.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    pub fn satisfies<T: AsRef<SysMediaStreamTrack>>(&self, track: T) -> bool {
        let track = track.as_ref();
        if track.kind() != "audio" {
            return false;
        }

        if track.ready_state() != MediaStreamTrackState::Live {
            return false;
        }

        satisfies_by_device_id(&self.device_id, track)
        // TODO returns Result<bool, Error>
    }

    /// Merges this [`AudioTrackConstraints`] with `another` one, meaning that
    /// if some constraint is not set on this one, then it will be applied from
    /// `another`.
    pub fn merge(&mut self, another: AudioTrackConstraints) {
        if self.device_id.is_none() && another.device_id.is_some() {
            self.device_id = another.device_id;
        }
        if !self.is_required && another.is_required {
            self.is_required = another.is_required;
        }
    }

    /// Returns importance of this [`AudioTrackConstraints`].
    ///
    /// If this [`AudioTrackConstraints`] is important then without this
    /// [`AudioTrackConstraints`] call session can't be started.
    pub fn is_required(&self) -> bool {
        self.is_required
    }
}

impl From<ProtoAudioConstraints> for AudioTrackConstraints {
    #[inline]
    fn from(caps: ProtoAudioConstraints) -> Self {
        Self {
            is_required: caps.is_required,
            device_id: None,
        }
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
#[derive(Clone, Debug, Default)]
pub struct VideoTrackConstraints {
    /// Constraints applicable to video tracks.
    constraints: Option<
        StreamSource<DeviceVideoTrackConstraints, DisplayVideoTrackConstraints>,
    >,

    /// Importance of this [`VideoTrackConstraints`].
    ///
    /// If `true` then without this [`VideoTrackConstraints`] call session
    /// can't be started.
    is_required: bool,
}

/// Representation of the [`ConstrainDOMString`].
///
/// Can set exact and ideal constrain value.
///
/// [`ConstrainDOMString`]: https://tinyurl.com/y6qkebfk
#[derive(Clone, Copy, Debug)]
struct StringConstrain<T> {
    /// If the property can't be set to one of the listed values, matching will
    /// fail.
    exact: Option<T>,

    /// If possible, one of the listed values will be used, but if it's not
    /// possible, the user agent will use the closest possible match.
    ideal: Option<T>,
}

impl<T> Default for StringConstrain<T> {
    fn default() -> Self {
        Self {
            exact: None,
            ideal: None,
        }
    }
}

impl<T> StringConstrain<T> {
    /// Sets `ideal` property of the constrain.
    ///
    /// If the property can't be set to one of the listed values, matching will
    /// fail.
    pub fn set_ideal(&mut self, ideal: T) {
        self.ideal = Some(ideal);
    }

    /// Sets `exact` property of the constrain.
    ///
    /// If possible, one of the listed values will be used, but if it's not
    /// possible, the user agent will use the closest possible match.
    pub fn set_exact(&mut self, exact: T) {
        self.exact = Some(exact);
    }

    /// Returns `true` if nothing was set in this [`StringConstrain`].
    pub fn is_empty(&self) -> bool {
        self.exact.is_none() && self.ideal.is_none()
    }
}

impl<T> From<&StringConstrain<T>> for SysConstrainDomStringParameters
where
    T: ToString,
{
    fn from(from: &StringConstrain<T>) -> Self {
        let mut constrain = SysConstrainDomStringParameters::new();
        if let Some(ideal) = &from.ideal {
            constrain.ideal(&(ideal.to_string().into()));
        }
        if let Some(exact) = &from.exact {
            constrain.exact(&(exact.to_string().into()));
        }

        constrain
    }
}

/// Describes the directions that the camera can face, as seen from the user's
/// perspective. Representation of [VideoFacingModeEnum][1].
///
/// [1]: https://www.w3.org/TR/mediacapture-streams/#dom-videofacingmodeenum
#[wasm_bindgen]
#[derive(Clone, Copy, PartialEq, Display, Debug)]
pub enum FacingMode {
    /// The source is facing toward the user (a self-view camera).
    #[display(fmt = "user")]
    User,

    ///  The source is facing away from the user (viewing the environment).
    #[display(fmt = "environment")]
    Environment,

    /// The source is facing to the left of the user.
    #[display(fmt = "left")]
    Left,

    /// The source is facing to the right of the user.
    #[display(fmt = "right")]
    Right,
}

/// Constraints applicable to video tracks that are sourced from some media
/// device.
#[wasm_bindgen]
#[derive(Clone, Debug, Default)]
pub struct DeviceVideoTrackConstraints {
    /// The identifier of the device generating the content for the media
    /// track.
    device_id: Option<String>,

    /// Importance of this [`DeviceVideoTrackConstraints`].
    ///
    /// If `true` then without this [`DeviceVideoTrackConstraints`] call
    /// session can't be started.
    is_required: bool,

    /// [facingMode][1] constraint.
    ///
    /// [1]: https://tinyurl.com/y2ks2mjj
    facing_mode: StringConstrain<FacingMode>,
}

impl DeviceVideoTrackConstraints {
    /// Merges this [`DeviceVideoTrackConstraints`] with `another` one , meaning
    /// that if some constraint is not set on this one, then it will be applied
    /// from `another`.
    fn merge(&mut self, another: DeviceVideoTrackConstraints) {
        if self.device_id.is_none() && another.device_id.is_some() {
            self.device_id = another.device_id;
        }
        if !self.is_required && another.is_required {
            self.is_required = another.is_required;
        }
    }

    /// Returns importance of this [`DeviceVideoTrackConstraints`].
    ///
    /// If this [`DeviceVideoTrackConstraints`] is important then without this
    /// [`DeviceVideoTrackConstraints`] call session can't be started.
    pub fn is_required(&self) -> bool {
        self.is_required
    }
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
    /// [1]: https://w3.org/TR/mediacapture-streams/#def-constraint-deviceId
    pub fn device_id(&mut self, device_id: String) {
        self.device_id = Some(device_id);
    }

    /// Sets exact [facingMode][1] constraint.
    ///
    /// [1]: https://tinyurl.com/y2ks2mjj
    pub fn exact_facing_mode(&mut self, facing_mode: FacingMode) {
        self.facing_mode.set_exact(facing_mode);
    }

    /// Sets ideal [facingMode][1] constraint.
    ///
    /// [1]: https://tinyurl.com/y2ks2mjj
    pub fn ideal_facing_mode(&mut self, facing_mode: FacingMode) {
        self.facing_mode.set_ideal(facing_mode);
    }
}

/// Constraints applicable to video tracks sourced from screen capture.
#[wasm_bindgen]
#[derive(Clone, Debug, Default)]
pub struct DisplayVideoTrackConstraints {}

impl DisplayVideoTrackConstraints {
    /// Merges this [`DisplayVideoTrackConstraints`] with `another` one, meaning
    /// that if some constraint is not set on this one, then it will be applied
    /// from `another`.
    #[allow(clippy::unused_self)]
    fn merge(&mut self, _: DisplayVideoTrackConstraints) {
        // no constraints => nothing to do here atm
    }
}

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
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    pub fn satisfies<T: AsRef<SysMediaStreamTrack>>(&self, track: T) -> bool {
        let track = track.as_ref();
        if track.kind() != "video" {
            return false;
        }

        if track.ready_state() != MediaStreamTrackState::Live {
            return false;
        }

        match &self.constraints {
            None => true,
            Some(StreamSource::Device(constraints)) => {
                satisfies_by_device_id(&constraints.device_id, track)
                    && !Self::guess_is_from_display(&track)
            }
            Some(StreamSource::Display(_)) => {
                Self::guess_is_from_display(&track)
            }
        }
    }

    /// Detects if video track captured from display searching
    /// [specific fields][1] in its settings. Only works in Chrome atm.
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

    /// Merges this [`VideoTrackConstraints`] with `another` one, meaning that
    /// if some constraint is not set on this one, then it will be applied from
    /// `another`.
    pub fn merge(&mut self, another: VideoTrackConstraints) {
        if !self.is_required && another.is_required {
            self.is_required = another.is_required;
        }
        match (self.constraints.as_mut(), another.constraints) {
            (None, Some(another)) => {
                self.constraints.replace(another);
            }
            (Some(this), Some(another)) => {
                this.merge(another);
            }
            _ => {}
        };
    }

    /// Returns importance of this [`VideoTrackConstraints`].
    ///
    /// If this [`VideoTrackConstraints`] is important then without this
    /// [`VideoTrackConstraints`] call session can't be started.
    pub fn is_required(&self) -> bool {
        self.is_required
    }
}

impl From<ProtoVideoConstraints> for VideoTrackConstraints {
    fn from(caps: ProtoVideoConstraints) -> Self {
        Self {
            constraints: None,
            is_required: caps.is_required,
        }
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
        if !track_constraints.facing_mode.is_empty() {
            constraints.facing_mode(&SysConstrainDomStringParameters::from(
                &track_constraints.facing_mode,
            ));
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
        Self {
            is_required: constraints.is_required,
            constraints: Some(StreamSource::Device(constraints)),
        }
    }
}

impl From<DisplayVideoTrackConstraints> for VideoTrackConstraints {
    fn from(constraints: DisplayVideoTrackConstraints) -> Self {
        Self {
            is_required: true,
            constraints: Some(StreamSource::Display(constraints)),
        }
    }
}

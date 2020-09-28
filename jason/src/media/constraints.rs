// TODO: Split to multiple modules.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use derive_more::AsRef;
use medea_client_api_proto::{
    AudioSettings as ProtoAudioConstraints, MediaType as ProtoTrackConstraints,
    MediaType,
};
use wasm_bindgen::prelude::*;
use web_sys::{
    ConstrainDomStringParameters,
    MediaStreamConstraints as SysMediaStreamConstraints,
    MediaStreamTrack as SysMediaStreamTrack, MediaStreamTrackState,
    MediaTrackConstraints as SysMediaTrackConstraints,
};

use crate::{peer::TransceiverKind, utils::get_property_by_name};

/// Local media stream for injecting into new created [`PeerConnection`]s.
#[derive(Clone, Debug, Default)]
pub struct LocalTracksConstraints(Rc<RefCell<MediaStreamSettings>>);

impl LocalTracksConstraints {
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

    #[inline]
    pub fn is_display_video_constrained(&self) -> bool {
        self.0.borrow().is_display_constrained()
    }

    #[inline]
    pub fn is_device_video_constrained(&self) -> bool {
        self.0.borrow().is_device_constrained()
    }
}

#[cfg(feature = "mockable")]
impl From<MediaStreamSettings> for LocalTracksConstraints {
    #[inline]
    fn from(from: MediaStreamSettings) -> Self {
        Self(Rc::new(RefCell::new(from)))
    }
}

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

/// [MediaStreamConstraints][1] for the audio media type.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamconstraints
#[derive(Clone, Debug)]
struct AudioMediaTracksSettings {
    /// Constraints applicable to video tracks.
    constraints: AudioTrackConstraints,

    /// Indicator whether audio is enabled and this constraints should be
    /// injected into `Peer`.
    is_enabled: bool,
}

impl Default for AudioMediaTracksSettings {
    #[inline]
    fn default() -> Self {
        Self {
            constraints: AudioTrackConstraints::default(),
            is_enabled: true,
        }
    }
}

// TODO: move to utils
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
        get_property_by_name(&settings, "logicalSurface", |val| val.as_string())
            .is_some()
    }
}

/// Returns `true` if provided [`SysMediaStreamTrack`] basically satisfies
/// [`VideoTrackConstraints`].
fn satisfies_track(track: &SysMediaStreamTrack, kind: TransceiverKind) -> bool {
    track.kind() == kind.as_str()
        || track.ready_state() != MediaStreamTrackState::Live
}

#[derive(Clone, Debug)]
pub struct VideoTrackConstraintsNew<C> {
    constraints: Option<C>,
    is_enabled: bool,
}

impl<C> Default for VideoTrackConstraintsNew<C>
where
    C: Default,
{
    fn default() -> Self {
        Self {
            constraints: Some(C::default()),
            is_enabled: true,
        }
    }
}

impl<C> VideoTrackConstraintsNew<C> {
    pub fn is_enabled(&self) -> bool {
        self.is_enabled
    }

    pub fn set(&mut self, cons: C) {
        self.is_enabled = true;
        self.constraints = Some(cons);
    }

    fn clear(&mut self) {
        self.constraints.take();
    }

    fn is_constrained(&self) -> bool {
        self.constraints.is_some()
    }

    fn constrain(&mut self, other: Self) {
        self.is_enabled &= other.is_enabled;
        self.constraints = other.constraints;
    }
}

impl VideoTrackConstraintsNew<DeviceVideoTrackConstraints> {
    /// Returns `true` if provided [`SysMediaStreamTrack`] satisfies device
    /// [`MediaSource`] from this [`VideoTrackConstraints`].
    pub fn satisfies(&self, track: &SysMediaStreamTrack) -> bool {
        self.constraints
            .as_ref()
            .filter(|_| self.is_enabled)
            .map(|device| device.satisfies(track))
            .unwrap_or(false)
    }
}

impl VideoTrackConstraintsNew<DisplayVideoTrackConstraints> {
    pub fn satisfies(&self, track: &SysMediaStreamTrack) -> bool {
        self.constraints
            .as_ref()
            .filter(|_| self.is_enabled)
            .map(|display| display.satisfies(track))
            .unwrap_or(false)
    }
}

/// [MediaStreamConstraints][1] wrapper.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamconstraints
#[wasm_bindgen]
#[derive(Clone, Debug)]
pub struct MediaStreamSettings {
    /// [MediaStreamConstraints][1] for the audio media type.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamconstraints
    audio: AudioMediaTracksSettings,

    device_video: VideoTrackConstraintsNew<DeviceVideoTrackConstraints>,

    display_video: VideoTrackConstraintsNew<DisplayVideoTrackConstraints>,
}

impl Default for MediaStreamSettings {
    fn default() -> Self {
        Self {
            audio: AudioMediaTracksSettings::default(),
            device_video: VideoTrackConstraintsNew::default(),
            display_video: VideoTrackConstraintsNew::default(),
        }
    }
}

#[wasm_bindgen]
impl MediaStreamSettings {
    /// Creates new [`MediaStreamConstraints`] with none constraints configured.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            audio: AudioMediaTracksSettings {
                constraints: AudioTrackConstraints::default(),
                is_enabled: false,
            },
            display_video: VideoTrackConstraintsNew {
                is_enabled: false,
                constraints: None,
            },
            device_video: VideoTrackConstraintsNew {
                is_enabled: false,
                constraints: None,
            },
        }
    }

    /// Returns `true` if video from device can be published.
    fn is_device_enabled(&self) -> bool {
        self.device_video.is_enabled()
    }

    /// Returns `true` if video from display can be published.
    fn is_display_enabled(&self) -> bool {
        self.display_video.is_enabled()
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
        self.device_video.set(constraints);
    }

    /// Set constraints that will be used to capture local video from user
    /// display.
    pub fn display_video(&mut self, constraints: DisplayVideoTrackConstraints) {
        self.display_video.set(constraints);
    }
}

impl MediaStreamSettings {
    /// Removes [`MediaSource`] from this [`VideoTrackConstraints`] if provided
    /// [`SysMediaStreamTrack`] satisfies [`MediaSource`].
    pub fn unconstrain_if_satisfies_video<T>(&mut self, track: T) -> bool
    where
        T: AsRef<SysMediaStreamTrack>,
    {
        let track = track.as_ref();
        if self.device_video.satisfies(track.as_ref()) {
            self.device_video.clear();
            true
        } else if self.display_video.satisfies(track.as_ref()) {
            self.display_video.clear();
            true
        } else {
            false
        }
    }

    pub fn is_device_constrained(&self) -> bool {
        self.device_video.is_constrained()
    }

    pub fn is_display_constrained(&self) -> bool {
        self.display_video.is_constrained()
    }

    /// Returns only audio constraints.
    #[inline]
    pub fn get_audio(&self) -> &AudioTrackConstraints {
        &self.audio.constraints
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

    /// Sets the underlying [`AudioMediaTracksSettings::is_enabled`] to the
    /// given value.
    #[inline]
    pub fn toggle_publish_audio(&mut self, is_enabled: bool) {
        self.audio.is_enabled = is_enabled;
    }

    /// Sets the underlying [`VideoMediaTracksSettings::is_enabled`] to the
    /// given value.
    #[inline]
    pub fn toggle_publish_video(&mut self, is_enabled: bool) {
        self.display_video.is_enabled = is_enabled;
        self.device_video.is_enabled = is_enabled;
    }

    /// Indicates whether audio is enabled in this [`MediaStreamSettings`].
    #[inline]
    pub fn is_audio_enabled(&self) -> bool {
        self.audio.is_enabled
    }

    /// Indicates whether video is enabled in this [`MediaStreamSettings`].
    #[inline]
    pub fn is_video_enabled(&self) -> bool {
        self.device_video.is_enabled() || self.display_video.is_enabled()
    }

    pub fn is_device_video_enabled(&self) -> bool {
        self.device_video.is_enabled
    }

    pub fn is_display_video_enabled(&self) -> bool {
        self.display_video.is_enabled
    }

    /// Indicates whether the given [`MediaType`] is enabled in this
    /// [`MediaStreamSettings`].
    #[inline]
    pub fn is_enabled(&self, kind: &MediaType) -> bool {
        match kind {
            MediaType::Video(video) => {
                if video.is_display {
                    self.display_video.is_enabled()
                } else {
                    self.device_video.is_enabled()
                }
            }
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
        self.audio.constraints = other.audio.constraints;
        self.display_video.constrain(other.display_video);
        self.device_video.constrain(other.device_video);
    }
}

// TODO: DisplayMediaStreamConstraints should be used when it will be
//       implemented by UA's.

/// Wrapper around [MediaStreamConstraints][1] that specifies concrete media
/// source (device or display), and allows to group two requests with different
/// sources.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#mediastreamconstraints
#[derive(Debug)]
pub enum MultiSourceTracksConstraints {
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
impl From<MediaStreamSettings> for Option<MultiSourceTracksConstraints> {
    fn from(constraints: MediaStreamSettings) -> Self {
        use MultiSourceTracksConstraints as C;

        let is_device_video_enabled = constraints.is_device_video_enabled();
        let is_display_video_enabled = constraints.is_display_video_enabled();
        let is_device_audio_enabled = constraints.is_audio_enabled();

        let mut device_cons = None;
        let mut display_cons = None;

        if is_device_video_enabled {
            if let Some(device_video_cons) =
                constraints.device_video.constraints
            {
                device_cons
                    .get_or_insert_with(SysMediaStreamConstraints::new)
                    .video(
                        &SysMediaTrackConstraints::from(device_video_cons)
                            .into(),
                    );
            }
        }
        if is_display_video_enabled {
            if let Some(display_video_cons) =
                constraints.display_video.constraints
            {
                display_cons
                    .get_or_insert_with(SysMediaStreamConstraints::new)
                    .video(
                        &SysMediaTrackConstraints::from(display_video_cons)
                            .into(),
                    );
            }
        }
        if is_device_audio_enabled {
            device_cons
                .get_or_insert_with(SysMediaStreamConstraints::new)
                .audio(
                    &SysMediaTrackConstraints::from(
                        constraints.audio.constraints,
                    )
                    .into(),
                );
        }

        match (device_cons, display_cons) {
            (Some(device_cons), Some(display_cons)) => {
                Some(MultiSourceTracksConstraints::DeviceAndDisplay(
                    device_cons,
                    display_cons,
                ))
            }
            (Some(device_cons), None) => {
                Some(MultiSourceTracksConstraints::Device(device_cons))
            }
            (None, Some(display_cons)) => {
                Some(MultiSourceTracksConstraints::Display(display_cons))
            }
            (None, None) => None,
        }
    }
}

/// Constraints for the [`MediaStreamTrack`].
#[derive(Clone, Debug)]
pub enum VideoSource {
    /// [`MediaStreamTrack`] should be received from the `getUserMedia`
    /// request.
    Device(DeviceVideoTrackConstraints),

    /// [`MediaStreamTrack`] should be received from the `getDisplayMedia`
    /// request.
    Display(DisplayVideoTrackConstraints),
}

impl VideoSource {
    /// Returns importance of this [`MediaStreamTrackConstraints`].
    ///
    /// If this [`MediaStreamTrackConstraints`] is important then without this
    /// [`MediaStreamTrackConstraints`] call session can't be started.
    pub fn is_required(&self) -> bool {
        match self {
            VideoSource::Device(device) => device.is_required,
            VideoSource::Display(display) => display.is_required,
        }
    }

    /// Checks if provided [MediaStreamTrack][1] satisfies this
    /// [`MediaStreamTrackConstraints`].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    pub fn satisfies<T: AsRef<SysMediaStreamTrack>>(&self, track: T) -> bool {
        let track = track.as_ref();
        match self {
            VideoSource::Display(display) => display.satisfies(&track),
            VideoSource::Device(device) => device.satisfies(track),
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
    Video(VideoSource),
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
            TrackConstraints::Video(video) => video.is_required(),
            TrackConstraints::Audio(audio) => audio.is_required,
        }
    }

    /// Returns `true` if this [`TrackConstraints`]'s media should be received
    /// from `getDisplayMedia`.
    pub fn is_display_video(&self) -> bool {
        matches!(self, Self::Video(VideoSource::Display(_)))
    }
}

impl From<ProtoTrackConstraints> for TrackConstraints {
    fn from(caps: ProtoTrackConstraints) -> Self {
        match caps {
            ProtoTrackConstraints::Audio(audio) => Self::Audio(audio.into()),
            ProtoTrackConstraints::Video(video) => {
                if video.is_display {
                    Self::Video(VideoSource::Display(
                        DisplayVideoTrackConstraints {
                            is_required: video.is_required,
                        },
                    ))
                } else {
                    Self::Video(VideoSource::Device(
                        DeviceVideoTrackConstraints {
                            device_id: None,
                            facing_mode: None,
                            is_required: video.is_required,
                        },
                    ))
                }
            }
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
    device_id: Option<ConstrainString<DeviceId>>,

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

    /// Sets exact [deviceId][1] constraint.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#def-constraint-deviceId
    pub fn device_id(&mut self, device_id: String) {
        self.device_id = Some(ConstrainString::Exact(DeviceId(device_id)));
    }
}

impl AudioTrackConstraints {
    /// Checks if provided [MediaStreamTrack][1] satisfies constraints
    /// contained.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    pub fn satisfies<T: AsRef<SysMediaStreamTrack>>(&self, track: T) -> bool {
        let track = track.as_ref();
        satisfies_track(track, TransceiverKind::Audio)
            && ConstrainString::satisfies(&self.device_id, track)
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
            constraints
                .device_id(&ConstrainDomStringParameters::from(&device_id));
        }

        constraints
    }
}

/// Constraints applicable to [MediaStreamTrack][1].
///
/// Constraints provide a general control surface that allows applications to
/// both select an appropriate source for a track and, once selected, to
/// influence how a source operates.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack
trait Constraint {
    /// Returns constrained parameter field name.
    fn track_settings_field_name() -> &'static str;
}

/// The identifier of the device generating the content of the
/// [MediaStreamTrack][1].
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack
#[derive(AsRef, Clone, Debug)]
#[as_ref(forward)]
struct DeviceId(String);

impl Constraint for DeviceId {
    fn track_settings_field_name() -> &'static str {
        "deviceId"
    }
}

/// Describes the directions that the camera can face, as seen from the user's
/// perspective. Representation of [VideoFacingModeEnum][1].
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-videofacingmodeenum
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FacingMode {
    /// Facing toward the user (a self-view camera).
    User,

    /// Facing away from the user (viewing the environment).
    Environment,

    /// Facing to the left of the user.
    Left,

    /// Facing to the right of the user.
    Right,
}

impl AsRef<str> for FacingMode {
    fn as_ref(&self) -> &str {
        match self {
            FacingMode::User => "user",
            FacingMode::Environment => "environment",
            FacingMode::Left => "left",
            FacingMode::Right => "right",
        }
    }
}

impl Constraint for FacingMode {
    fn track_settings_field_name() -> &'static str {
        "facingMode"
    }
}

/// Representation of the [ConstrainDOMString][1].
///
/// Can set exact (must be the parameter's value) and ideal (should be used if
/// possible) constrain.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-constraindomstring
#[derive(Clone, Copy, Debug)]
enum ConstrainString<T> {
    Exact(T),
    Ideal(T),
}

impl<T: Constraint + AsRef<str>> ConstrainString<T> {
    fn satisfies(this: &Option<Self>, track: &SysMediaStreamTrack) -> bool {
        match this {
            None | Some(ConstrainString::Ideal(_)) => true,
            Some(ConstrainString::Exact(constrain)) => get_property_by_name(
                &track.get_settings(),
                T::track_settings_field_name(),
                |v| v.as_string(),
            )
            .map_or(false, |id| id.as_str() == constrain.as_ref()),
        }
    }
}

impl<T: AsRef<str>> From<&ConstrainString<T>> for ConstrainDomStringParameters {
    fn from(from: &ConstrainString<T>) -> Self {
        let mut constraint = ConstrainDomStringParameters::new();
        match from {
            ConstrainString::Exact(val) => {
                constraint.exact(&JsValue::from_str(val.as_ref()))
            }
            ConstrainString::Ideal(val) => {
                constraint.ideal(&JsValue::from_str(val.as_ref()))
            }
        };

        constraint
    }
}

/// Constraints applicable to video tracks that are sourced from some media
/// device.
#[wasm_bindgen]
#[derive(Clone, Debug, Default)]
pub struct DeviceVideoTrackConstraints {
    /// Importance of this [`DeviceVideoTrackConstraints`].
    ///
    /// If `true` then without this [`DeviceVideoTrackConstraints`] call
    /// session can't be started.
    is_required: bool,

    /// The identifier of the device generating the content for the media
    /// track.
    device_id: Option<ConstrainString<DeviceId>>,

    /// Describes the directions that the camera can face, as seen from the
    /// user's perspective.
    facing_mode: Option<ConstrainString<FacingMode>>,
}

impl DeviceVideoTrackConstraints {
    /// Returns importance of this [`DeviceVideoTrackConstraints`].
    ///
    /// If this [`DeviceVideoTrackConstraints`] is important then without this
    /// [`DeviceVideoTrackConstraints`] call session can't be started.
    pub fn is_required(&self) -> bool {
        self.is_required
    }

    pub fn satisfies(&self, track: &SysMediaStreamTrack) -> bool {
        satisfies_track(track, TransceiverKind::Video)
            && ConstrainString::satisfies(&self.device_id, track)
            && ConstrainString::satisfies(&self.facing_mode, track)
            && !guess_is_from_display(&track)
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

    /// Sets exact [deviceId][1] constraint.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#def-constraint-deviceId
    pub fn device_id(&mut self, device_id: String) {
        self.device_id = Some(ConstrainString::Exact(DeviceId(device_id)));
    }

    /// Sets exact [facingMode][1] constraint.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#dom-constraindomstring
    pub fn exact_facing_mode(&mut self, facing_mode: FacingMode) {
        self.facing_mode = Some(ConstrainString::Exact(facing_mode));
    }

    /// Sets ideal [facingMode][1] constraint.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#dom-constraindomstring
    pub fn ideal_facing_mode(&mut self, facing_mode: FacingMode) {
        self.facing_mode = Some(ConstrainString::Ideal(facing_mode));
    }
}

/// Constraints applicable to video tracks sourced from screen capture.
#[wasm_bindgen]
#[derive(Clone, Debug, Default)]
pub struct DisplayVideoTrackConstraints {
    is_required: bool,
}

impl DisplayVideoTrackConstraints {
    pub fn satisfies(&self, track: &SysMediaStreamTrack) -> bool {
        satisfies_track(track, TransceiverKind::Video)
            && guess_is_from_display(&track)
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

impl From<DeviceVideoTrackConstraints> for SysMediaTrackConstraints {
    fn from(track_constraints: DeviceVideoTrackConstraints) -> Self {
        let mut constraints = Self::new();

        if let Some(device_id) = track_constraints.device_id {
            constraints
                .device_id(&ConstrainDomStringParameters::from(&device_id));
        }
        if let Some(facing_mode) = track_constraints.facing_mode {
            constraints
                .facing_mode(&ConstrainDomStringParameters::from(&facing_mode));
        }

        constraints
    }
}

impl From<DisplayVideoTrackConstraints> for SysMediaTrackConstraints {
    fn from(_: DisplayVideoTrackConstraints) -> Self {
        Self::new()
    }
}

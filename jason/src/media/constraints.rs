// TODO: Split to multiple modules.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use derive_more::{AsRef, Into};
use medea_client_api_proto::{
    AudioSettings as ProtoAudioConstraints, MediaSourceKind,
    MediaType as ProtoTrackConstraints, MediaType, VideoSettings,
};
use wasm_bindgen::prelude::*;
use web_sys as sys;

use crate::{
    media::MediaKind,
    peer::{
        media_exchange_state, mute_state, LocalStreamUpdateCriteria, MediaState,
    },
    utils::get_property_by_name,
};

/// Local media stream for injecting into new created [`PeerConnection`]s.
///
/// [`PeerConnection`]: crate::peer::PeerConnection
#[derive(Clone, Debug, Default)]
pub struct LocalTracksConstraints(Rc<RefCell<MediaStreamSettings>>);

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
    pub fn set_enabled(&self, enabled: bool, kind: MediaKind) {
        match kind {
            MediaKind::Audio => {
                self.is_audio_enabled.set(enabled);
            }
            MediaKind::Video => {
                self.is_video_enabled.set(enabled);
            }
        }
    }

    /// Returns is audio receiving enabled.
    #[inline]
    pub fn is_audio_enabled(&self) -> bool {
        self.is_audio_enabled.get()
    }

    /// Returns is video receiving enabled.
    #[inline]
    pub fn is_video_enabled(&self) -> bool {
        self.is_video_enabled.get()
    }
}

#[cfg(feature = "mockable")]
impl From<MediaStreamSettings> for LocalTracksConstraints {
    #[inline]
    fn from(from: MediaStreamSettings) -> Self {
        Self(Rc::new(RefCell::new(from)))
    }
}

impl LocalTracksConstraints {
    /// Returns [`LocalStreamUpdateCriteria`] with [`MediaKind`] and
    /// [`MediaSourceKind`] which are different in the provided
    /// [`MediaStreamSettings`].
    #[inline]
    #[must_use]
    pub fn calculate_kinds_diff(
        &self,
        settings: &MediaStreamSettings,
    ) -> LocalStreamUpdateCriteria {
        self.0.borrow().calculate_kinds_diff(&settings)
    }

    /// Constrains the underlying [`MediaStreamSettings`] with the given `other`
    /// [`MediaStreamSettings`].
    #[inline]
    pub fn constrain(&self, other: MediaStreamSettings) {
        self.0.borrow_mut().constrain(other)
    }

    /// Clones the underlying [`MediaStreamSettings`].
    #[inline]
    #[must_use]
    pub fn inner(&self) -> MediaStreamSettings {
        self.0.borrow().clone()
    }

    /// Changes the underlying [`MediaStreamSettings`] basing on the provided
    /// [`MediaState`].
    #[inline]
    pub fn set_media_state(
        &self,
        state: MediaState,
        kind: MediaKind,
        source_kind: Option<MediaSourceKind>,
    ) {
        self.0
            .borrow_mut()
            .set_track_media_state(state, kind, source_kind);
    }

    /// Enables/disables provided [`LocalStreamUpdateCriteria`] based on
    /// provided [`media_exchange_state`].
    #[inline]
    pub fn set_media_exchange_state_by_kinds(
        &self,
        state: media_exchange_state::Stable,
        kinds: LocalStreamUpdateCriteria,
    ) {
        self.0
            .borrow_mut()
            .set_media_exchange_state_by_kinds(state, kinds)
    }

    /// Indicates whether provided [`MediaType`] is enabled in the underlying
    /// [`MediaStreamSettings`].
    #[inline]
    #[must_use]
    pub fn enabled(&self, kind: &MediaType) -> bool {
        self.0.borrow().enabled(kind)
    }

    /// Indicates whether provided [`MediaType`] is muted in the underlying
    /// [`MediaStreamSettings`].
    #[inline]
    #[must_use]
    pub fn muted(&self, kind: &MediaType) -> bool {
        self.0.borrow().muted(kind)
    }

    /// Indicates whether provided [`MediaKind`] and [`MediaSourceKind`] are
    /// enabled in this [`LocalTracksConstraints`].
    #[inline]
    #[must_use]
    pub fn is_track_enabled(
        &self,
        kind: MediaKind,
        source: MediaSourceKind,
    ) -> bool {
        self.0.borrow().is_track_enabled(kind, source)
    }
}

/// [MediaStreamConstraints][1] for the audio media type.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamconstraints
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AudioMediaTracksSettings {
    /// Constraints applicable to video tracks.
    constraints: AudioTrackConstraints,

    /// Indicator whether audio is enabled and this constraints should be
    /// injected into `Peer`.
    enabled: bool,

    /// Indicator whether audio should be muted.
    muted: bool,
}

impl Default for AudioMediaTracksSettings {
    #[inline]
    fn default() -> Self {
        Self {
            constraints: AudioTrackConstraints::default(),
            enabled: true,
            muted: false,
        }
    }
}

/// Returns `true` if provided [`sys::MediaStreamTrack`] basically satisfies any
/// constraints with a provided [`MediaKind`].
#[inline]
fn satisfies_track(track: &sys::MediaStreamTrack, kind: MediaKind) -> bool {
    track.kind() == kind.as_str()
        && track.ready_state() == sys::MediaStreamTrackState::Live
}

/// [MediaStreamConstraints][1] for the video media type.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamconstraints
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VideoTrackConstraints<C> {
    /// Constraints applicable to video tracks.
    ///
    /// If [`None`] then this kind of video (device or display) is disabled by
    /// [`MediaStreamSettings`].
    constraints: Option<C>,

    /// Indicator whether video is enabled and this constraints should be
    /// injected into `Peer`.
    ///
    /// Any action with this flag should be performed only while disable/enable
    /// actions by [`Room`]. This flag can't be changed by
    /// [`MediaStreamSettings`] updating.
    ///
    /// [`Room`]: crate::api::Room
    enabled: bool,

    /// Indicator whether video should be muted.
    muted: bool,
}

impl<C: Default> Default for VideoTrackConstraints<C> {
    fn default() -> Self {
        Self {
            constraints: Some(C::default()),
            enabled: true,
            muted: false,
        }
    }
}

impl<C> VideoTrackConstraints<C> {
    /// Returns `true` if this [`VideoTrackConstraints`] are enabled by the
    /// [`Room`] and constrained with [`VideoTrackConstraints::constraints`].
    ///
    /// [`Room`]: crate::api::Room
    #[inline]
    fn enabled(&self) -> bool {
        self.enabled && self.is_constrained()
    }

    /// Sets these [`VideoTrackConstraints::constraints`] to the provided
    /// `cons`.
    #[inline]
    fn set(&mut self, cons: C) {
        self.constraints = Some(cons);
    }

    /// Resets these [`VideoTrackConstraints::constraints`] to [`None`].
    #[inline]
    fn unconstrain(&mut self) {
        self.constraints.take();
    }

    /// Returns `true` if these [`VideoTrackConstraints::constraints`] are set
    /// to [`Some`] value.
    #[inline]
    fn is_constrained(&self) -> bool {
        self.constraints.is_some()
    }

    /// Constraints these [`VideoTrackConstraints`] with a provided `other`
    /// [`VideoTrackConstraints`].
    #[inline]
    fn constrain(&mut self, other: Self) {
        self.constraints = other.constraints;
    }
}

impl VideoTrackConstraints<DeviceVideoTrackConstraints> {
    /// Indicates whether the provided [`sys::MediaStreamTrack`] satisfies
    /// device [`VideoTrackConstraints::constraints`].
    ///
    /// Returns `false` if [`VideoTrackConstraints::constraints`] is not set.
    fn satisfies(&self, track: &sys::MediaStreamTrack) -> bool {
        self.constraints
            .as_ref()
            .filter(|_| self.enabled())
            .map_or(false, |device| device.satisfies(track))
    }
}

impl VideoTrackConstraints<DisplayVideoTrackConstraints> {
    /// Indicates whether the provided [`sys::MediaStreamTrack`] satisfies
    /// device [`VideoTrackConstraints::constraints`].
    ///
    /// Returns `false` if [`VideoTrackConstraints::constraints`] is not set.
    fn satisfies(&self, track: &sys::MediaStreamTrack) -> bool {
        self.constraints
            .as_ref()
            .filter(|_| self.enabled())
            .map_or(false, |display| display.satisfies(track))
    }
}

/// [MediaStreamConstraints][1] wrapper.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamconstraints
#[wasm_bindgen]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MediaStreamSettings {
    /// [MediaStreamConstraints][1] for the audio media type.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamconstraints
    audio: AudioMediaTracksSettings,

    /// [MediaStreamConstraints][1] for the device video media type.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamconstraints
    device_video: VideoTrackConstraints<DeviceVideoTrackConstraints>,

    /// [MediaStreamConstraints][1] for the display video media type.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamconstraints
    display_video: VideoTrackConstraints<DisplayVideoTrackConstraints>,
}

#[wasm_bindgen]
impl MediaStreamSettings {
    /// Creates new [`MediaStreamSettings`] with none constraints configured.
    #[must_use]
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            audio: AudioMediaTracksSettings {
                constraints: AudioTrackConstraints::default(),
                enabled: false,
                muted: false,
            },
            display_video: VideoTrackConstraints {
                enabled: true,
                constraints: None,
                muted: false,
            },
            device_video: VideoTrackConstraints {
                enabled: true,
                constraints: None,
                muted: false,
            },
        }
    }

    /// Specifies the nature and settings of the audio [MediaStreamTrack][1].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    pub fn audio(&mut self, constraints: AudioTrackConstraints) {
        self.audio.enabled = true;
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
    /// Returns `true` if provided [`sys::MediaStreamTrack`] satisfies some of
    /// the [`VideoTrackConstraints`] from this [`MediaStreamSettings`].
    ///
    /// Unconstrains [`VideoTrackConstraints`] which this
    /// [`sys::MediaStreamTrack`] satisfies by calling
    /// [`VideoTrackConstraints::unconstrain`].
    pub fn unconstrain_if_satisfies_video<T>(&mut self, track: T) -> bool
    where
        T: AsRef<sys::MediaStreamTrack>,
    {
        let track = track.as_ref();
        if self.device_video.satisfies(track.as_ref()) {
            self.device_video.unconstrain();
            true
        } else if self.display_video.satisfies(track.as_ref()) {
            self.display_video.unconstrain();
            true
        } else {
            false
        }
    }

    /// Returns [`LocalStreamUpdateCriteria`] with [`MediaKind`] and
    /// [`MediaSourceKind`] which are different in the provided
    /// [`MediaStreamSettings`].
    #[must_use]
    pub fn calculate_kinds_diff(
        &self,
        another: &Self,
    ) -> LocalStreamUpdateCriteria {
        let mut kinds = LocalStreamUpdateCriteria::empty();
        if self.device_video != another.device_video {
            kinds.add(MediaKind::Video, MediaSourceKind::Device);
        }
        if self.display_video != another.display_video {
            kinds.add(MediaKind::Video, MediaSourceKind::Display);
        }
        if self.audio != another.audio {
            kinds.add(MediaKind::Audio, MediaSourceKind::Device);
        }

        kinds
    }

    /// Returns only audio constraints.
    #[inline]
    #[must_use]
    pub fn get_audio(&self) -> &AudioTrackConstraints {
        &self.audio.constraints
    }

    /// Returns reference to [`DisplayVideoTrackConstraints`] from this
    /// [`MediaStreamSettings`].
    ///
    /// Returns [`None`] if [`DisplayVideoTrackConstraints`] is unconstrained.
    #[inline]
    #[must_use]
    pub fn get_display_video(&self) -> Option<&DisplayVideoTrackConstraints> {
        self.display_video.constraints.as_ref()
    }

    /// Returns reference to [`DeviceVideoTrackConstraints`] from this
    /// [`MediaStreamSettings`].
    ///
    /// Returns [`None`] if [`DeviceVideoTrackConstraints`] is unconstrained.
    #[inline]
    #[must_use]
    pub fn get_device_video(&self) -> Option<&DeviceVideoTrackConstraints> {
        self.device_video.constraints.as_ref()
    }

    /// Changes [`MediaState`] of audio or video type in this
    /// [`MediaStreamSettings`].
    ///
    /// If some type of the [`MediaStreamSettings`] is disabled, then this kind
    /// of media won't be published.
    #[inline]
    pub fn set_track_media_state(
        &mut self,
        state: MediaState,
        kind: MediaKind,
        source_kind: Option<MediaSourceKind>,
    ) {
        match kind {
            MediaKind::Audio => match state {
                MediaState::Mute(muted) => {
                    self.set_audio_muted(muted == mute_state::Stable::Muted);
                }
                MediaState::MediaExchange(media_exchange) => {
                    self.set_audio_publish(
                        media_exchange == media_exchange_state::Stable::Enabled,
                    );
                }
            },
            MediaKind::Video => match state {
                MediaState::Mute(muted) => {
                    self.set_video_muted(
                        muted == mute_state::Stable::Muted,
                        source_kind,
                    );
                }
                MediaState::MediaExchange(media_exchange) => {
                    self.set_video_publish(
                        media_exchange == media_exchange_state::Stable::Enabled,
                        source_kind,
                    );
                }
            },
        }
    }

    /// Enables/disables provided [`LocalStreamUpdateCriteria`] based on
    /// provided [`media_exchange_state`].
    #[inline]
    pub fn set_media_exchange_state_by_kinds(
        &mut self,
        state: media_exchange_state::Stable,
        kinds: LocalStreamUpdateCriteria,
    ) {
        let enabled = state == media_exchange_state::Stable::Enabled;
        if kinds.has(MediaKind::Audio, MediaSourceKind::Device) {
            self.set_audio_publish(enabled);
        }
        if kinds.has(MediaKind::Video, MediaSourceKind::Device) {
            self.set_video_publish(enabled, Some(MediaSourceKind::Device));
        }
        if kinds.has(MediaKind::Video, MediaSourceKind::Display) {
            self.set_video_publish(enabled, Some(MediaSourceKind::Display));
        }
    }

    /// Sets the underlying [`AudioMediaTracksSettings::muted`] to the provided
    /// value.
    fn set_audio_muted(&mut self, muted: bool) {
        self.audio.muted = muted;
    }

    /// Sets the underlying [`VideoTrackConstraints::muted`] basing on the
    /// provided [`MediaSourceKind`] to the given value.
    fn set_video_muted(
        &mut self,
        muted: bool,
        source_kind: Option<MediaSourceKind>,
    ) {
        match source_kind {
            None => {
                self.display_video.muted = muted;
                self.device_video.muted = muted;
            }
            Some(MediaSourceKind::Device) => {
                self.device_video.muted = muted;
            }
            Some(MediaSourceKind::Display) => {
                self.display_video.muted = muted;
            }
        }
    }

    /// Sets the underlying [`AudioMediaTracksSettings::enabled`] to the
    /// given value.
    #[inline]
    pub fn set_audio_publish(&mut self, enabled: bool) {
        self.audio.enabled = enabled;
    }

    /// Sets underlying [`VideoTrackConstraints::enabled`] based on provided
    /// [`MediaSourceKind`] to the given value.
    #[inline]
    pub fn set_video_publish(
        &mut self,
        enabled: bool,
        source_kind: Option<MediaSourceKind>,
    ) {
        match source_kind {
            None => {
                self.display_video.enabled = enabled;
                self.device_video.enabled = enabled;
            }
            Some(MediaSourceKind::Device) => {
                self.device_video.enabled = enabled;
            }
            Some(MediaSourceKind::Display) => {
                self.display_video.enabled = enabled;
            }
        }
    }

    /// Indicates whether audio is enabled in this [`MediaStreamSettings`].
    #[inline]
    #[must_use]
    pub fn is_audio_enabled(&self) -> bool {
        self.audio.enabled
    }

    /// Returns `true` if [`DeviceVideoTrackConstraints`] are currently
    /// constrained and enabled.
    #[inline]
    #[must_use]
    pub fn is_device_video_enabled(&self) -> bool {
        self.device_video.enabled()
    }

    /// Returns `true` if [`DisplayVideoTrackConstraints`] are currently
    /// constrained and enabled.
    #[inline]
    #[must_use]
    pub fn is_display_video_enabled(&self) -> bool {
        self.display_video.enabled()
    }

    /// Indicates whether the given [`MediaType`] is enabled and constrained in
    /// this [`MediaStreamSettings`].
    #[inline]
    #[must_use]
    pub fn enabled(&self, kind: &MediaType) -> bool {
        match kind {
            MediaType::Video(video) => {
                self.is_track_enabled(MediaKind::Video, video.source_kind)
            }
            MediaType::Audio(_) => {
                self.is_track_enabled(MediaKind::Audio, MediaSourceKind::Device)
            }
        }
    }

    /// Indicates whether the given [`MediaType`] is muted in this
    /// [`MediaStreamSettings`].
    #[inline]
    #[must_use]
    pub fn muted(&self, kind: &MediaType) -> bool {
        match kind {
            MediaType::Video(video) => match video.source_kind {
                MediaSourceKind::Device => self.device_video.muted,
                MediaSourceKind::Display => self.display_video.muted,
            },
            MediaType::Audio(_) => self.audio.muted,
        }
    }

    /// Indicates whether the given [`MediaKind`] and [`MediaSourceKind`] are
    /// enabled in this [`MediaStreamSettings`].
    #[inline]
    #[must_use]
    pub fn is_track_enabled(
        &self,
        kind: MediaKind,
        source: MediaSourceKind,
    ) -> bool {
        match (kind, source) {
            (MediaKind::Video, MediaSourceKind::Device) => {
                self.device_video.enabled()
            }
            (MediaKind::Video, MediaSourceKind::Display) => {
                self.display_video.enabled()
            }
            (MediaKind::Audio, _) => self.audio.enabled,
        }
    }

    /// Constrains this [`MediaStreamSettings`] with the given `other`
    /// [`MediaStreamSettings`].
    #[inline]
    fn constrain(&mut self, other: Self) {
        // `&=` cause we should not enable disabled Room, but we can disable
        // enabled room.
        self.audio.enabled &= other.audio.enabled;
        self.audio.constraints = other.audio.constraints;
        self.display_video.constrain(other.display_video);
        self.device_video.constrain(other.device_video);
    }
}

/// Wrapper around [MediaStreamConstraints][1] that specifies concrete media
/// source (device or display), and allows to group two requests with different
/// sources.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#mediastreamconstraints
#[derive(Debug)]
pub enum MultiSourceTracksConstraints {
    /// Only [getUserMedia()][1] request is required.
    ///
    /// [1]: https://tinyurl.com/w3-streams#dom-mediadevices-getusermedia
    Device(sys::MediaStreamConstraints),

    /// Only [getDisplayMedia()][1] request is required.
    ///
    /// [1]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
    Display(sys::DisplayMediaStreamConstraints),

    /// Both [getUserMedia()][1] and [getDisplayMedia()][2] are required.
    ///
    /// [1]: https://tinyurl.com/w3-streams#dom-mediadevices-getusermedia
    /// [2]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
    DeviceAndDisplay(
        sys::MediaStreamConstraints,
        sys::DisplayMediaStreamConstraints,
    ),
}

impl From<MediaStreamSettings> for Option<MultiSourceTracksConstraints> {
    fn from(constraints: MediaStreamSettings) -> Self {
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
                    .get_or_insert_with(sys::MediaStreamConstraints::new)
                    .video(
                        &sys::MediaTrackConstraints::from(device_video_cons)
                            .into(),
                    );
            }
        }
        if is_display_video_enabled {
            if let Some(display_video_cons) =
                constraints.display_video.constraints
            {
                display_cons
                    .get_or_insert_with(sys::DisplayMediaStreamConstraints::new)
                    .video(
                        &sys::MediaTrackConstraints::from(display_video_cons)
                            .into(),
                    );
            }
        }
        if is_device_audio_enabled {
            device_cons
                .get_or_insert_with(sys::MediaStreamConstraints::new)
                .audio(
                    &sys::MediaTrackConstraints::from(
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

/// Constraints for the [`MediaKind::Video`] [`local::Track`].
///
/// [`local::Track`]: crate::media::track::local::Track
#[derive(Clone, Debug)]
pub enum VideoSource {
    /// [`local::Track`] should be received from the `getUserMedia` request.
    ///
    /// [`local::Track`]: crate::media::track::local::Track
    Device(DeviceVideoTrackConstraints),

    /// [`local::Track`] should be received from the `getDisplayMedia` request.
    ///
    /// [`local::Track`]: crate::media::track::local::Track
    Display(DisplayVideoTrackConstraints),
}

impl VideoSource {
    /// Returns importance of this [`VideoSource`].
    ///
    /// If this [`VideoSource`] is important then without this [`VideoSource`]
    /// call session can't be started.
    #[inline]
    #[must_use]
    pub fn required(&self) -> bool {
        match self {
            VideoSource::Device(device) => device.required,
            VideoSource::Display(display) => display.required,
        }
    }

    /// Checks if provided [MediaStreamTrack][1] satisfies this [`VideoSource`].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    #[inline]
    pub fn satisfies<T: AsRef<sys::MediaStreamTrack>>(&self, track: T) -> bool {
        let track = track.as_ref();
        match self {
            VideoSource::Display(display) => display.satisfies(&track),
            VideoSource::Device(device) => device.satisfies(track),
        }
    }
}

impl From<VideoSettings> for VideoSource {
    fn from(settings: VideoSettings) -> Self {
        match settings.source_kind {
            MediaSourceKind::Device => {
                VideoSource::Device(DeviceVideoTrackConstraints {
                    device_id: None,
                    facing_mode: None,
                    width: None,
                    height: None,
                    required: settings.required,
                })
            }
            MediaSourceKind::Display => {
                VideoSource::Display(DisplayVideoTrackConstraints {
                    required: settings.required,
                })
            }
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
    pub fn satisfies<T: AsRef<sys::MediaStreamTrack>>(&self, track: T) -> bool {
        match self {
            Self::Audio(audio) => audio.satisfies(&track),
            Self::Video(video) => video.satisfies(&track),
        }
    }

    /// Returns importance of this [`TrackConstraints`].
    ///
    /// If this [`TrackConstraints`] is important then without this
    /// [`TrackConstraints`] call session can't be started.
    #[must_use]
    pub fn required(&self) -> bool {
        match self {
            TrackConstraints::Video(video) => video.required(),
            TrackConstraints::Audio(audio) => audio.required,
        }
    }

    /// Returns this [`TrackConstraints`] media source kind.
    #[must_use]
    pub fn media_source_kind(&self) -> MediaSourceKind {
        match &self {
            TrackConstraints::Audio(_) => MediaSourceKind::Device,
            TrackConstraints::Video(VideoSource::Device(_)) => {
                MediaSourceKind::Device
            }
            TrackConstraints::Video(VideoSource::Display(_)) => {
                MediaSourceKind::Display
            }
        }
    }

    /// Returns [`MediaKind`] of these [`TrackConstraints`].
    #[must_use]
    pub fn media_kind(&self) -> MediaKind {
        match &self {
            TrackConstraints::Audio(_) => MediaKind::Audio,
            TrackConstraints::Video(_) => MediaKind::Video,
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
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AudioTrackConstraints {
    /// The identifier of the device generating the content for the media
    /// track.
    device_id: Option<ConstrainString<DeviceId>>,

    /// Importance of this [`AudioTrackConstraints`].
    ///
    /// If `true` then without this [`AudioTrackConstraints`] call session
    /// can't be started.
    required: bool,
}

#[wasm_bindgen]
impl AudioTrackConstraints {
    /// Creates new [`AudioTrackConstraints`] with none constraints configured.
    #[must_use]
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
    pub fn satisfies<T: AsRef<sys::MediaStreamTrack>>(&self, track: T) -> bool {
        let track = track.as_ref();
        satisfies_track(track, MediaKind::Audio)
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
        if !self.required && another.required {
            self.required = another.required;
        }
    }

    /// Returns importance of this [`AudioTrackConstraints`].
    ///
    /// If this [`AudioTrackConstraints`] is important then without this
    /// [`AudioTrackConstraints`] call session can't be started.
    #[must_use]
    pub fn required(&self) -> bool {
        self.required
    }
}

impl From<ProtoAudioConstraints> for AudioTrackConstraints {
    #[inline]
    fn from(caps: ProtoAudioConstraints) -> Self {
        Self {
            required: caps.required,
            device_id: None,
        }
    }
}

impl From<AudioTrackConstraints> for sys::MediaTrackConstraints {
    fn from(track_constraints: AudioTrackConstraints) -> Self {
        let mut constraints = Self::new();

        if let Some(device_id) = track_constraints.device_id {
            constraints.device_id(&sys::ConstrainDomStringParameters::from(
                &device_id,
            ));
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
    /// Constrained parameter field name.
    const TRACK_SETTINGS_FIELD_NAME: &'static str;
}

/// The identifier of the device generating the content of the
/// [MediaStreamTrack][1].
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack
#[derive(AsRef, Clone, Debug, Eq, PartialEq)]
#[as_ref(forward)]
struct DeviceId(String);

impl Constraint for DeviceId {
    const TRACK_SETTINGS_FIELD_NAME: &'static str = "deviceId";
}

/// Height, in pixels, of the video.
#[derive(Clone, Copy, Debug, Eq, Into, PartialEq)]
struct Height(u32);

impl Constraint for Height {
    const TRACK_SETTINGS_FIELD_NAME: &'static str = "height";
}

/// Width, in pixels, of the video.
#[derive(Clone, Copy, Debug, Eq, Into, PartialEq)]
struct Width(u32);

impl Constraint for Width {
    const TRACK_SETTINGS_FIELD_NAME: &'static str = "width";
}

/// Describes the directions that the camera can face, as seen from the user's
/// perspective. Representation of [VideoFacingModeEnum][1].
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-videofacingmodeenum
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
    const TRACK_SETTINGS_FIELD_NAME: &'static str = "facingMode";
}

/// Representation of the [ConstrainULong][1]. Underlying value must fit in
/// `[0, 4294967295]` range.
///
/// [1]: https://tinyurl.com/w3-streams#dom-constrainulong
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConstrainU32<T> {
    /// Must be the parameter's value.
    Exact(T),

    /// Should be used if possible.
    Ideal(T),

    /// Parameter's value must be in this range.
    Range(T, T),
}

impl<T: Constraint + Into<u32>> ConstrainU32<T> {
    // It's up to `<T as Constraint>::TRACK_SETTINGS_FIELD_NAME` to guarantee
    // that such casts are safe.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn satisfies(this: Option<Self>, track: &sys::MediaStreamTrack) -> bool {
        match this {
            None | Some(ConstrainU32::Ideal(_)) => true,
            Some(ConstrainU32::Exact(exact)) => get_property_by_name(
                &track.get_settings(),
                T::TRACK_SETTINGS_FIELD_NAME,
                |v| v.as_f64().map(|v| v as u32),
            )
            .map_or(false, |val| val == exact.into()),
            Some(ConstrainU32::Range(start, end)) => get_property_by_name(
                &track.get_settings(),
                T::TRACK_SETTINGS_FIELD_NAME,
                |v| v.as_f64().map(|v| v as u32),
            )
            .map_or(false, |val| val >= start.into() && val <= end.into()),
        }
    }
}

impl<T: Constraint + Into<u32>> From<ConstrainU32<T>>
    for sys::ConstrainDoubleRange
{
    fn from(from: ConstrainU32<T>) -> Self {
        let mut constraint = sys::ConstrainDoubleRange::new();
        match from {
            ConstrainU32::Exact(val) => {
                constraint.exact(f64::from(val.into()));
            }
            ConstrainU32::Ideal(val) => {
                constraint.ideal(f64::from(val.into()));
            }
            ConstrainU32::Range(min, max) => {
                constraint
                    .min(f64::from(min.into()))
                    .max(f64::from(max.into()));
            }
        }

        constraint
    }
}

/// Representation of the [ConstrainDOMString][1].
///
/// Can set exact (must be the parameter's value) and ideal (should be used if
/// possible) constrain.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-constraindomstring
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConstrainString<T> {
    Exact(T),
    Ideal(T),
}

impl<T: Constraint + AsRef<str>> ConstrainString<T> {
    fn satisfies(this: &Option<Self>, track: &sys::MediaStreamTrack) -> bool {
        match this {
            None | Some(ConstrainString::Ideal(_)) => true,
            Some(ConstrainString::Exact(constrain)) => get_property_by_name(
                &track.get_settings(),
                T::TRACK_SETTINGS_FIELD_NAME,
                |v| v.as_string(),
            )
            .map_or(false, |id| id.as_str() == constrain.as_ref()),
        }
    }
}

impl<T: AsRef<str>> From<&ConstrainString<T>>
    for sys::ConstrainDomStringParameters
{
    fn from(from: &ConstrainString<T>) -> Self {
        let mut constraint = sys::ConstrainDomStringParameters::new();
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
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DeviceVideoTrackConstraints {
    /// Importance of this [`DeviceVideoTrackConstraints`].
    ///
    /// If `true` then without this [`DeviceVideoTrackConstraints`] call
    /// session can't be started.
    required: bool,

    /// The identifier of the device generating the content for the media
    /// track.
    device_id: Option<ConstrainString<DeviceId>>,

    /// Describes the directions that the camera can face, as seen from the
    /// user's perspective.
    facing_mode: Option<ConstrainString<FacingMode>>,

    /// Height of the video in pixels.
    height: Option<ConstrainU32<Height>>,

    /// Width of the video in pixels.
    width: Option<ConstrainU32<Width>>,
}

impl DeviceVideoTrackConstraints {
    /// Checks if provided [MediaStreamTrack][1] satisfies
    /// [`DeviceVideoTrackConstraints`] contained.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    #[must_use]
    pub fn satisfies(&self, track: &sys::MediaStreamTrack) -> bool {
        satisfies_track(track, MediaKind::Video)
            && ConstrainString::satisfies(&self.device_id, track)
            && ConstrainString::satisfies(&self.facing_mode, track)
            && ConstrainU32::satisfies(self.height, track)
            && ConstrainU32::satisfies(self.width, track)
            && !guess_is_from_display(&track)
    }

    /// Merges this [`DeviceVideoTrackConstraints`] with `another` one , meaning
    /// that if some constraint is not set on this one, then it will be applied
    /// from `another`.
    pub fn merge(&mut self, another: DeviceVideoTrackConstraints) {
        if self.device_id.is_none() && another.device_id.is_some() {
            self.device_id = another.device_id;
        }
        if !self.required && another.required {
            self.required = another.required;
        }
        if self.facing_mode.is_none() && another.facing_mode.is_some() {
            self.facing_mode = another.facing_mode;
        }
        if self.height.is_none() && another.height.is_some() {
            self.height = another.height;
        }
        if self.width.is_none() && another.width.is_some() {
            self.width = another.width;
        }
    }

    /// Returns importance of this [`DeviceVideoTrackConstraints`].
    ///
    /// If this [`DeviceVideoTrackConstraints`] is important then without this
    /// [`DeviceVideoTrackConstraints`] call session can't be started.
    #[inline]
    #[must_use]
    pub fn required(&self) -> bool {
        self.required
    }
}

/// Constraints applicable to video tracks that are sourced from screen-capture.
#[wasm_bindgen]
impl DeviceVideoTrackConstraints {
    /// Creates new [`DeviceVideoTrackConstraints`] with none constraints
    /// configured.
    #[must_use]
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

    /// Sets exact [`height`][1] constraint.
    ///
    /// [1]: https://tinyurl.com/w3-streams#def-constraint-height
    pub fn exact_height(&mut self, height: u32) {
        self.height = Some(ConstrainU32::Exact(Height(height)));
    }

    /// Sets ideal [`height`][1] constraint.
    ///
    /// [1]: https://tinyurl.com/w3-streams#def-constraint-height
    pub fn ideal_height(&mut self, height: u32) {
        self.height = Some(ConstrainU32::Ideal(Height(height)));
    }

    /// Sets range of [`height`][1] constraint.
    ///
    /// [1]: https://tinyurl.com/w3-streams#def-constraint-height
    pub fn height_in_range(&mut self, min: u32, max: u32) {
        self.height = Some(ConstrainU32::Range(Height(min), Height(max)));
    }

    /// Sets exact [`width`][1] constraint.
    ///
    /// [1]: https://tinyurl.com/w3-streams#def-constraint-width
    pub fn exact_width(&mut self, width: u32) {
        self.width = Some(ConstrainU32::Exact(Width(width)));
    }

    /// Sets ideal [`width`][1] constraint.
    ///
    /// [1]: https://tinyurl.com/w3-streams#def-constraint-width
    pub fn ideal_width(&mut self, width: u32) {
        self.width = Some(ConstrainU32::Ideal(Width(width)));
    }

    /// Sets range of [`width`][1] constraint.
    ///
    /// [1]: https://tinyurl.com/w3-streams#def-constraint-width
    pub fn width_in_range(&mut self, min: u32, max: u32) {
        self.width = Some(ConstrainU32::Range(Width(min), Width(max)));
    }
}

/// Constraints applicable to video tracks sourced from screen capture.
#[wasm_bindgen]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DisplayVideoTrackConstraints {
    /// Importance of this [`DisplayVideoTrackConstraints`].
    ///
    /// If `true` then without this [`DisplayVideoTrackConstraints`] call
    /// session can't be started.
    required: bool,
}

impl DisplayVideoTrackConstraints {
    /// Checks if provided [MediaStreamTrack][1] satisfies
    /// [`DisplayVideoTrackConstraints`] contained.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    #[allow(clippy::unused_self)]
    #[inline]
    #[must_use]
    pub fn satisfies(&self, track: &sys::MediaStreamTrack) -> bool {
        satisfies_track(track, MediaKind::Video)
            && guess_is_from_display(&track)
    }

    /// Merges this [`DisplayVideoTrackConstraints`] with `another` one,
    /// meaning that if some constraint is not set on this one, then it will be
    /// applied from `another`.
    #[inline]
    pub fn merge(&mut self, another: &Self) {
        if !self.required && another.required {
            self.required = another.required;
        }
    }

    /// Returns importance of this [`DisplayVideoTrackConstraints`].
    ///
    /// If this [`DisplayVideoTrackConstraints`] is important then without this
    /// [`DisplayVideoTrackConstraints`] call session can't be started.
    #[inline]
    #[must_use]
    pub fn required(&self) -> bool {
        self.required
    }
}

#[wasm_bindgen]
impl DisplayVideoTrackConstraints {
    /// Creates new [`DisplayVideoTrackConstraints`] with none constraints
    /// configured.
    #[must_use]
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }
}

/// Detects if video track captured from display searching [specific fields][1]
/// in its settings. Only works in Chrome atm.
///
/// [1]: https://w3.org/TR/screen-capture/#extensions-to-mediatracksettings
fn guess_is_from_display(track: &sys::MediaStreamTrack) -> bool {
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

impl From<DeviceVideoTrackConstraints> for sys::MediaTrackConstraints {
    fn from(track_constraints: DeviceVideoTrackConstraints) -> Self {
        let mut constraints = Self::new();

        if let Some(device_id) = track_constraints.device_id {
            constraints.device_id(&sys::ConstrainDomStringParameters::from(
                &device_id,
            ));
        }
        if let Some(facing_mode) = track_constraints.facing_mode {
            constraints.facing_mode(&sys::ConstrainDomStringParameters::from(
                &facing_mode,
            ));
        }
        if let Some(width) = track_constraints.width {
            constraints.width(&sys::ConstrainDoubleRange::from(width));
        }
        if let Some(height) = track_constraints.height {
            constraints.height(&sys::ConstrainDoubleRange::from(height));
        }

        constraints
    }
}

impl From<DisplayVideoTrackConstraints> for sys::MediaTrackConstraints {
    fn from(_: DisplayVideoTrackConstraints) -> Self {
        Self::new()
    }
}

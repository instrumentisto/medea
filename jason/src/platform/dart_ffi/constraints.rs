//! Media tracks and streams constraints functionality.

use crate::media::{
    constraints::{ConstrainString, ConstrainU32},
    AudioTrackConstraints, DeviceVideoTrackConstraints,
    DisplayVideoTrackConstraints,
};
use derive_more::{AsRef, Into};
use web_sys::{
    ConstrainDomStringParameters, ConstrainDoubleRange, MediaTrackConstraints,
};

/// [MediaStreamConstraints][1] wrapper.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamconstraints
#[derive(AsRef, Debug, Into)]
pub struct MediaStreamConstraints(web_sys::MediaStreamConstraints);

impl MediaStreamConstraints {
    /// Creates new [`MediaStreamConstraints`] with none constraints configured.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self(web_sys::MediaStreamConstraints::new())
    }

    /// Specifies the nature and settings of the `audio` [MediaStreamTrack][1].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    #[inline]
    pub fn audio(&mut self, audio: AudioTrackConstraints) {
        self.0.audio(&MediaTrackConstraints::from(audio).into());
    }

    /// Specifies the nature and settings of the `video` [MediaStreamTrack][1].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    #[inline]
    pub fn video(&mut self, video: DeviceVideoTrackConstraints) {
        self.0.video(&MediaTrackConstraints::from(video).into());
    }
}

impl Default for MediaStreamConstraints {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl From<AudioTrackConstraints> for MediaTrackConstraints {
    fn from(track_constraints: AudioTrackConstraints) -> Self {
        unimplemented!()
    }
}

impl From<DeviceVideoTrackConstraints> for MediaTrackConstraints {
    fn from(track_constraints: DeviceVideoTrackConstraints) -> Self {
        unimplemented!()
    }
}

impl From<ConstrainU32> for ConstrainDoubleRange {
    fn from(from: ConstrainU32) -> Self {
        unimplemented!()
    }
}

impl<T: AsRef<str>> From<&ConstrainString<T>> for ConstrainDomStringParameters {
    fn from(from: &ConstrainString<T>) -> Self {
        unimplemented!()
    }
}

/// [DisplayMediaStreamConstraints][1] wrapper.
///
/// [1]: https://w3.org/TR/screen-capture/#dom-displaymediastreamconstraints
#[derive(AsRef, Debug, Into)]
pub struct DisplayMediaStreamConstraints();

impl Default for DisplayMediaStreamConstraints {
    #[inline]
    fn default() -> Self {
        unimplemented!()
    }
}

impl DisplayMediaStreamConstraints {
    /// Creates a new [`DisplayMediaStreamConstraints`] with none constraints
    /// configured.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        unimplemented!()
    }

    /// Specifies the nature and settings of the `video` [MediaStreamTrack][1].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    #[inline]
    pub fn video(&mut self, video: DisplayVideoTrackConstraints) {
        unimplemented!()
    }
}

impl From<DisplayVideoTrackConstraints> for MediaTrackConstraints {
    #[inline]
    fn from(_: DisplayVideoTrackConstraints) -> Self {
        unimplemented!()
    }
}

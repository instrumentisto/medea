//! Media tracks and streams constraints functionality.

use crate::media::{
    AudioTrackConstraints, DeviceVideoTrackConstraints,
    DisplayVideoTrackConstraints,
};
use derive_more::{AsRef, Into};

/// [MediaStreamConstraints][1] wrapper.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamconstraints
#[derive(AsRef, Debug, Into)]
pub struct MediaStreamConstraints;

impl MediaStreamConstraints {
    /// Creates new [`MediaStreamConstraints`] with none constraints configured.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        unimplemented!()
    }

    /// Specifies the nature and settings of the `audio` [MediaStreamTrack][1].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    #[inline]
    pub fn audio(&mut self, audio: AudioTrackConstraints) {
        unimplemented!()
    }

    /// Specifies the nature and settings of the `video` [MediaStreamTrack][1].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    #[inline]
    pub fn video(&mut self, video: DeviceVideoTrackConstraints) {
        unimplemented!()
    }
}

impl Default for MediaStreamConstraints {
    #[inline]
    fn default() -> Self {
        Self::new()
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

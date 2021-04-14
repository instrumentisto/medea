use dart_sys::Dart_Handle;

use crate::media::{
    constraints::ConstrainU32, AudioTrackConstraints,
    DeviceVideoTrackConstraints, DisplayVideoTrackConstraints,
};

#[derive(Clone, Debug)]
pub struct MediaStreamConstraints(Dart_Handle);

impl MediaStreamConstraints {
    pub fn new() -> Self {
        todo!()
    }

    pub fn audio(&mut self, audio: AudioTrackConstraints) {
        todo!()
    }

    pub fn video(&mut self, video: DeviceVideoTrackConstraints) {
        todo!()
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
pub struct DisplayMediaStreamConstraints(Dart_Handle);

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
        todo!()
    }

    /// Specifies the nature and settings of the `video` [MediaStreamTrack][1].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    #[inline]
    pub fn video(&mut self, video: DisplayVideoTrackConstraints) {
        todo!()
    }
}

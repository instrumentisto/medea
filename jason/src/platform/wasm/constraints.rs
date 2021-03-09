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
        let mut constraints = Self::new();

        if let Some(device_id) = track_constraints.device_id {
            constraints
                .device_id(&ConstrainDomStringParameters::from(&device_id));
        }

        constraints
    }
}

impl From<DeviceVideoTrackConstraints> for MediaTrackConstraints {
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
        if let Some(width) = track_constraints.width {
            constraints.width(&ConstrainDoubleRange::from(width));
        }
        if let Some(height) = track_constraints.height {
            constraints.height(&ConstrainDoubleRange::from(height));
        }

        constraints
    }
}

impl From<ConstrainU32> for ConstrainDoubleRange {
    fn from(from: ConstrainU32) -> Self {
        let mut constraint = ConstrainDoubleRange::new();
        match from {
            ConstrainU32::Exact(val) => {
                constraint.exact(f64::from(val));
            }
            ConstrainU32::Ideal(val) => {
                constraint.ideal(f64::from(val));
            }
            ConstrainU32::Range(min, max) => {
                constraint.min(f64::from(min)).max(f64::from(max));
            }
        }

        constraint
    }
}

impl<T: AsRef<str>> From<&ConstrainString<T>> for ConstrainDomStringParameters {
    fn from(from: &ConstrainString<T>) -> Self {
        let mut constraint = ConstrainDomStringParameters::new();
        match from {
            ConstrainString::Exact(val) => {
                constraint.exact(&wasm_bindgen::JsValue::from_str(val.as_ref()))
            }
            ConstrainString::Ideal(val) => {
                constraint.ideal(&wasm_bindgen::JsValue::from_str(val.as_ref()))
            }
        };

        constraint
    }
}

/// [DisplayMediaStreamConstraints][1] wrapper.
///
/// [1]: https://w3.org/TR/screen-capture/#dom-displaymediastreamconstraints
#[derive(AsRef, Debug, Into)]
pub struct DisplayMediaStreamConstraints(
    web_sys::DisplayMediaStreamConstraints,
);

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
        Self(web_sys::DisplayMediaStreamConstraints::new())
    }

    /// Specifies the nature and settings of the `video` [MediaStreamTrack][1].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    #[inline]
    pub fn video(&mut self, video: DisplayVideoTrackConstraints) {
        self.0.video(&MediaTrackConstraints::from(video).into());
    }
}

impl From<DisplayVideoTrackConstraints> for MediaTrackConstraints {
    #[inline]
    fn from(_: DisplayVideoTrackConstraints) -> Self {
        Self::new()
    }
}

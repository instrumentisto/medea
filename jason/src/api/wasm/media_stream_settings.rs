//! [MediaStreamConstraints][1] wrapper.
//!
//! [1]: https://w3.org/TR/mediacapture-streams#dom-mediastreamconstraints

#![allow(clippy::new_without_default)]

use derive_more::{From, Into};
use wasm_bindgen::prelude::*;

use crate::{api::FacingMode, media};

/// [MediaStreamConstraints][1] wrapper.
///
/// [1]: https://w3.org/TR/mediacapture-streams#dom-mediastreamconstraints
#[wasm_bindgen]
#[derive(Clone, From, Into)]
pub struct MediaStreamSettings(media::MediaStreamSettings);

#[wasm_bindgen]
impl MediaStreamSettings {
    /// Creates new [`MediaStreamSettings`] with none constraints configured.
    #[must_use]
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        media::MediaStreamSettings::new().into()
    }

    /// Specifies the nature and settings of an audio [MediaStreamTrack][1].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams#mediastreamtrack
    pub fn audio(&mut self, constraints: AudioTrackConstraints) {
        self.0.audio(constraints.into())
    }

    /// Set constraints that will be used to obtain a local video sourced from
    /// a media device.
    pub fn device_video(&mut self, constraints: DeviceVideoTrackConstraints) {
        self.0.device_video(constraints.into());
    }

    /// Set constraints that will be used to capture a local video from a user's
    /// display.
    pub fn display_video(&mut self, constraints: DisplayVideoTrackConstraints) {
        self.0.display_video(constraints.into());
    }
}

/// Constraints applicable to audio tracks.
#[wasm_bindgen]
#[derive(From, Into)]
pub struct AudioTrackConstraints(media::AudioTrackConstraints);

#[wasm_bindgen]
impl AudioTrackConstraints {
    /// Creates new [`AudioTrackConstraints`] with none constraints configured.
    #[must_use]
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        media::AudioTrackConstraints::new().into()
    }

    /// Sets an exact [deviceId][1] constraint.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams#def-constraint-deviceId
    pub fn device_id(&mut self, device_id: String) {
        self.0.device_id(device_id);
    }
}

/// Constraints applicable to video tracks that are sourced from some media
/// device.
#[wasm_bindgen]
#[derive(From, Into)]
pub struct DeviceVideoTrackConstraints(media::DeviceVideoTrackConstraints);

/// Constraints applicable to video tracks that are sourced from a screen
/// capturing.
#[wasm_bindgen]
impl DeviceVideoTrackConstraints {
    /// Creates new [`DeviceVideoTrackConstraints`] with none constraints
    /// configured.
    #[must_use]
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        media::DeviceVideoTrackConstraints::new().into()
    }

    /// Sets an exact [deviceId][1] constraint.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams#def-constraint-deviceId
    pub fn device_id(&mut self, device_id: String) {
        self.0.device_id(device_id);
    }

    /// Sets an exact [facingMode][1] constraint.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams#dom-constraindomstring
    pub fn exact_facing_mode(&mut self, facing_mode: FacingMode) {
        self.0.exact_facing_mode(facing_mode.into());
    }

    /// Sets an ideal [facingMode][1] constraint.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams#dom-constraindomstring
    pub fn ideal_facing_mode(&mut self, facing_mode: FacingMode) {
        self.0.ideal_facing_mode(facing_mode.into())
    }

    /// Sets an exact [`height`][1] constraint.
    ///
    /// [1]: https://tinyurl.com/w3-streams#def-constraint-height
    pub fn exact_height(&mut self, height: u32) {
        self.0.exact_height(height)
    }

    /// Sets an ideal [`height`][1] constraint.
    ///
    /// [1]: https://tinyurl.com/w3-streams#def-constraint-height
    pub fn ideal_height(&mut self, height: u32) {
        self.0.ideal_height(height);
    }

    /// Sets a range of a [`height`][1] constraint.
    ///
    /// [1]: https://tinyurl.com/w3-streams#def-constraint-height
    pub fn height_in_range(&mut self, min: u32, max: u32) {
        self.0.height_in_range(min, max);
    }

    /// Sets an exact [`width`][1] constraint.
    ///
    /// [1]: https://tinyurl.com/w3-streams#def-constraint-width
    pub fn exact_width(&mut self, width: u32) {
        self.0.exact_width(width);
    }

    /// Sets an ideal [`width`][1] constraint.
    ///
    /// [1]: https://tinyurl.com/w3-streams#def-constraint-width
    pub fn ideal_width(&mut self, width: u32) {
        self.0.ideal_width(width);
    }

    /// Sets a range of a [`width`][1] constraint.
    ///
    /// [1]: https://tinyurl.com/w3-streams#def-constraint-width
    pub fn width_in_range(&mut self, min: u32, max: u32) {
        self.0.width_in_range(min, max);
    }
}

/// Constraints applicable to video tracks sourced from a screen capturing.
#[wasm_bindgen]
#[derive(From, Into)]
pub struct DisplayVideoTrackConstraints(media::DisplayVideoTrackConstraints);

#[wasm_bindgen]
impl DisplayVideoTrackConstraints {
    /// Creates new [`DisplayVideoTrackConstraints`] with none constraints
    /// configured.
    #[must_use]
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        media::DisplayVideoTrackConstraints::new().into()
    }
}

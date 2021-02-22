use derive_more::From;
use wasm_bindgen::prelude::*;

use crate::platform;

/// Representation of [MediaDeviceInfo][1].
///
/// [1]: https://w3.org/TR/mediacapture-streams/#device-info
#[wasm_bindgen]
#[derive(From)]
pub struct InputDeviceInfo(platform::InputDeviceInfo);

#[wasm_bindgen]
impl InputDeviceInfo {
    /// Returns unique identifier for the represented device.
    pub fn device_id(&self) -> String {
        self.0.device_id()
    }

    /// Returns kind of the represented device.
    ///
    /// This representation of [MediaDeviceInfo][1] ONLY for input device.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#device-info
    pub fn kind(&self) -> super::MediaKind {
        self.0.kind().into()
    }

    /// Returns label describing the represented device (for example
    /// "External USB Webcam").
    /// If the device has no associated label, then returns an empty string.
    pub fn label(&self) -> String {
        self.0.label()
    }

    /// Returns group identifier of the represented device.
    ///
    /// Two devices have the same group identifier if they belong to the same
    /// physical device. For example, the audio input and output devices
    /// representing the speaker and microphone of the same headset have the
    /// same [groupId][1].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediadeviceinfo-groupid
    pub fn group_id(&self) -> String {
        self.0.group_id()
    }
}

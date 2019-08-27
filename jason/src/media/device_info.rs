//! [`MediaDeviceInfo`][1] related objects.
//!
//! [1]: https://www.w3.org/TR/mediacapture-streams/#device-info

use wasm_bindgen::prelude::*;
use web_sys::{MediaDeviceInfo as SysMediaDeviceInfo, MediaDeviceKind};

/// Representation of [MediaDeviceInfo][1].
///
/// [1]: https://www.w3.org/TR/mediacapture-streams/#device-info
#[allow(clippy::module_name_repetitions)]
#[wasm_bindgen]
pub struct MediaDeviceInfo {
    /// Actual underlying [MediaDeviceInfo][1] object.
    ///
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#device-info
    info: SysMediaDeviceInfo,
}

#[wasm_bindgen]
impl MediaDeviceInfo {
    /// A unique identifier for the represented device.
    #[wasm_bindgen(getter = deviceId)]
    pub fn device_id(&self) -> String {
        self.info.device_id()
    }

    /// Describes the kind of the represented device.
    ///
    /// This representation of [`MediaDeviceInfo`] ONLY for input device.
    #[wasm_bindgen(getter)]
    pub fn kind(&self) -> String {
        match self.info.kind() {
            MediaDeviceKind::Audioinput => "audio".to_string(),
            MediaDeviceKind::Videoinput => "video".to_string(),
            _ => unreachable!(),
        }
    }

    /// A label describing this device (for example "External USB Webcam").
    /// If the device has no associated label, then returns the empty string.
    #[wasm_bindgen(getter)]
    pub fn label(&self) -> String {
        self.info.label()
    }

    /// The group identifier of the represented device. Two devices have the
    /// same group identifier if they belong to the same physical device.
    /// For example, the audio input and output devices representing the speaker
    /// and microphone of the same headset have the same groupId.
    #[wasm_bindgen(getter = groupId)]
    pub fn group_id(&self) -> String {
        self.info.group_id()
    }
}

impl From<SysMediaDeviceInfo> for MediaDeviceInfo {
    fn from(info: SysMediaDeviceInfo) -> Self {
        Self { info }
    }
}

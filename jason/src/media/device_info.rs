//! [`MediaDeviceInfo`][1] related objects.
//!
//! [1]: https://www.w3.org/TR/mediacapture-streams/#device-info

use std::convert::TryFrom;

use wasm_bindgen::prelude::*;
use web_sys::{MediaDeviceInfo, MediaDeviceKind};

use crate::utils::WasmErr;

/// Representation of [MediaDeviceInfo][1].
///
/// [1]: https://www.w3.org/TR/mediacapture-streams/#device-info
#[allow(clippy::module_name_repetitions)]
#[wasm_bindgen]
pub struct InputDeviceInfo {
    device_type: InputDeviceKind,

    /// Actual underlying [MediaDeviceInfo][1] object.
    ///
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#device-info
    info: MediaDeviceInfo,
}

/// [MediaDeviceKind][1] wrapper, excluding audiooutput.
///
/// [1]: https://www.w3.org/TR/mediacapture-streams/#dom-mediadevicekind
enum InputDeviceKind {
    Audio,
    Video,
}

impl InputDeviceKind {
    fn to_str(&self) -> &str {
        match self {
            Self::Audio => "audio",
            Self::Video => "video",
        }
    }
}

impl TryFrom<MediaDeviceKind> for InputDeviceKind {
    type Error = WasmErr;

    fn try_from(value: MediaDeviceKind) -> Result<Self, Self::Error> {
        match value {
            MediaDeviceKind::Audioinput => Ok(Self::Audio),
            MediaDeviceKind::Videoinput => Ok(Self::Video),
            _ => Err(WasmErr::from("Not input device")),
        }
    }
}

#[wasm_bindgen]
impl InputDeviceInfo {
    /// A unique identifier for the represented device.
    pub fn device_id(&self) -> String {
        self.info.device_id()
    }

    /// Describes the kind of the represented device.
    ///
    /// This representation of [`MediaDeviceInfo`] ONLY for input device.
    pub fn kind(&self) -> String {
        self.device_type.to_str().to_owned()
    }

    /// A label describing this device (for example "External USB Webcam").
    /// If the device has no associated label, then returns the empty string.
    pub fn label(&self) -> String {
        self.info.label()
    }

    /// The group identifier of the represented device. Two devices have the
    /// same group identifier if they belong to the same physical device.
    /// For example, the audio input and output devices representing the speaker
    /// and microphone of the same headset have the same groupId.
    pub fn group_id(&self) -> String {
        self.info.group_id()
    }
}

impl TryFrom<MediaDeviceInfo> for InputDeviceInfo {
    type Error = WasmErr;

    fn try_from(info: MediaDeviceInfo) -> Result<Self, Self::Error> {
        Ok(Self {
            device_type: InputDeviceKind::try_from(info.kind())?,
            info,
        })
    }
}

//! [`MediaDeviceInfo`][1] related objects.
//!
//! [1]: https://w3.org/TR/mediacapture-streams/#device-info

use std::convert::TryFrom;

use derive_more::Display;
use wasm_bindgen::prelude::*;
use web_sys::{MediaDeviceInfo, MediaDeviceKind};

/// Errors that may occur when parsing [`MediaDeviceInfo`][1].
///
/// [1]: https://w3.org/TR/mediacapture-streams/#device-info
#[derive(Debug, Display)]
pub enum Error {
    /// Occurs when kind of media device not input device.
    #[display(fmt = "Not an input device")]
    NotInputDevice,
}

/// Representation of [MediaDeviceInfo][1].
///
/// [1]: https://w3.org/TR/mediacapture-streams/#device-info
#[allow(clippy::module_name_repetitions)]
#[wasm_bindgen]
pub struct InputDeviceInfo {
    device_type: InputDeviceKind,

    /// Actual underlying [MediaDeviceInfo][1] object.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#device-info
    info: MediaDeviceInfo,
}

/// [MediaDeviceKind][1] wrapper, excluding `audiooutput`.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediadevicekind
enum InputDeviceKind {
    /// `audioinput` device (for example a microphone).
    Audio,
    /// `videoinput` device ( for example a webcam).
    Video,
}

impl InputDeviceKind {
    #[inline]
    fn as_str(&self) -> &str {
        match self {
            Self::Audio => "audio",
            Self::Video => "video",
        }
    }
}

impl TryFrom<MediaDeviceKind> for InputDeviceKind {
    type Error = Error;

    fn try_from(value: MediaDeviceKind) -> Result<Self, Self::Error> {
        match value {
            MediaDeviceKind::Audioinput => Ok(Self::Audio),
            MediaDeviceKind::Videoinput => Ok(Self::Video),
            _ => Err(Error::NotInputDevice),
        }
    }
}

#[wasm_bindgen]
impl InputDeviceInfo {
    /// Returns unique identifier for the represented device.
    pub fn device_id(&self) -> String {
        self.info.device_id()
    }

    /// Returns kind of the represented device.
    ///
    /// This representation of [MediaDeviceInfo][1] ONLY for input device.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#device-info
    pub fn kind(&self) -> String {
        self.device_type.as_str().to_owned()
    }

    /// Returns label describing the represented device (for example
    /// "External USB Webcam").
    /// If the device has no associated label, then returns an empty string.
    pub fn label(&self) -> String {
        self.info.label()
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
        self.info.group_id()
    }
}

impl TryFrom<MediaDeviceInfo> for InputDeviceInfo {
    type Error = Error;

    fn try_from(info: MediaDeviceInfo) -> Result<Self, Self::Error> {
        Ok(Self {
            device_type: InputDeviceKind::try_from(info.kind())?,
            info,
        })
    }
}

//! [MediaDeviceInfo][1] related objects.
//!
//! [1]: https://w3.org/TR/mediacapture-streams/#device-info

use derive_more::Display;

use crate::media::MediaKind;

/// Errors that may occur when parsing [MediaDeviceInfo][1].
///
/// [1]: https://w3.org/TR/mediacapture-streams/#device-info
#[derive(Debug, Display)]
pub enum Error {
    /// Occurs when kind of media device is not an input device.
    #[display(fmt = "Not an input device")]
    NotInputDevice,
}

/// Representation of [MediaDeviceInfo][1].
///
/// [1]: https://w3.org/TR/mediacapture-streams/#device-info
pub struct InputDeviceInfo {}

impl InputDeviceInfo {
    /// Returns unique identifier for the represented device.
    #[inline]
    #[must_use]
    pub fn device_id(&self) -> String {
        unimplemented!()
    }

    /// Returns kind of the represented device.
    ///
    /// This representation of [MediaDeviceInfo][1] ONLY for input device.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#device-info
    #[inline]
    #[must_use]
    pub fn kind(&self) -> MediaKind {
        unimplemented!()
    }

    /// Returns label describing the represented device (for example
    /// "External USB Webcam").
    /// If the device has no associated label, then returns an empty string.
    #[inline]
    #[must_use]
    pub fn label(&self) -> String {
        unimplemented!()
    }

    /// Returns group identifier of the represented device.
    ///
    /// Two devices have the same group identifier if they belong to the same
    /// physical device. For example, the audio input and output devices
    /// representing the speaker and microphone of the same headset have the
    /// same [groupId][1].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediadeviceinfo-groupid
    #[inline]
    #[must_use]
    pub fn group_id(&self) -> String {
        unimplemented!()
    }
}

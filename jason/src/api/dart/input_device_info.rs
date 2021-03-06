use std::{os::raw::c_char, ptr};

use super::{utils::string_into_c_str, ForeignClass};

use crate::media::MediaKind;

#[cfg(feature = "mockable")]
pub use self::mock::InputDeviceInfo;
#[cfg(not(feature = "mockable"))]
pub use crate::platform::InputDeviceInfo;

impl ForeignClass for InputDeviceInfo {}

/// Returns unique identifier of the represented device.
#[no_mangle]
pub unsafe extern "C" fn InputDeviceInfo__device_id(
    this: ptr::NonNull<InputDeviceInfo>,
) -> ptr::NonNull<c_char> {
    string_into_c_str(this.as_ref().device_id())
}

/// Returns kind of the represented device.
///
/// This representation of [MediaDeviceInfo][1] ONLY for input device.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#device-info
#[no_mangle]
pub unsafe extern "C" fn InputDeviceInfo__kind(
    this: ptr::NonNull<InputDeviceInfo>,
) -> MediaKind {
    this.as_ref().kind()
}

/// Returns label describing the represented device (for example "External USB
/// Webcam").
///
/// If the device has no associated label, then returns an empty string.
#[no_mangle]
pub unsafe extern "C" fn InputDeviceInfo__label(
    this: ptr::NonNull<InputDeviceInfo>,
) -> ptr::NonNull<c_char> {
    string_into_c_str(this.as_ref().label())
}

/// Returns group identifier of the represented device.
///
/// Two devices have the same group identifier if they belong to the same
/// physical device. For example, the audio input and output devices
/// representing the speaker and microphone of the same headset have the
/// same [groupId][1].
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediadeviceinfo-groupid
#[no_mangle]
pub unsafe extern "C" fn InputDeviceInfo__group_id(
    this: ptr::NonNull<InputDeviceInfo>,
) -> ptr::NonNull<c_char> {
    string_into_c_str(this.as_ref().group_id())
}

/// Frees the data behind the provided pointer.
///
/// # Safety
///
/// Should be called when object is no longer needed. Calling this more than
/// once for the same pointer is equivalent to double free.
#[no_mangle]
pub unsafe extern "C" fn InputDeviceInfo__free(
    this: ptr::NonNull<InputDeviceInfo>,
) {
    let _ = InputDeviceInfo::from_ptr(this);
}

#[cfg(feature = "mockable")]
mod mock {
    use crate::media::MediaKind;

    pub struct InputDeviceInfo;

    impl InputDeviceInfo {
        pub fn device_id(&self) -> String {
            String::from("InputDeviceInfo.device_id")
        }

        pub fn kind(&self) -> MediaKind {
            MediaKind::Audio
        }

        pub fn label(&self) -> String {
            String::from("InputDeviceInfo.label")
        }

        pub fn group_id(&self) -> String {
            String::from("InputDeviceInfo.group_id")
        }
    }
}

//! [MediaDevices][1] functionality.
//!
//! [1]: https://w3.org/TR/mediacapture-streams#mediadevices

use tracerr::Traced;

use crate::platform::{
    DisplayMediaStreamConstraints, Error, InputDeviceInfo,
    MediaStreamConstraints, MediaStreamTrack,
};

/// Collects information about the User Agent's available media input devices.
///
/// Adapter for a [MediaDevices.enumerateDevices()][1] function.
///
/// # Errors
///
/// With [`MediaManagerError::EnumerateDevicesFailed`] if
/// [MediaDevices.enumerateDevices()][1] returns error.
///
/// [1]: https://tinyurl.com/w3-streams#dom-mediadevices-enumeratedevices
/// [2]: https://w3.org/TR/mediacapture-streams#mediadevices
#[allow(clippy::unused_async)]
pub async fn enumerate_devices() -> Result<Vec<InputDeviceInfo>, Traced<Error>>
{
    unimplemented!()
}

/// Prompts a user for a permission to use a media input which produces vector
/// of [`MediaStreamTrack`]s containing the requested types of media.
///
/// Adapter for a [MediaDevices.getUserMedia()][1] function.
///
/// # Errors
///
/// With [`MediaManagerError::GetUserMediaFailed`] if
/// [MediaDevices.getUserMedia()][1] returns error.
///
/// [1]: https://w3.org/TR/mediacapture-streams#dom-mediadevices-getusermedia
/// [2]: https://w3.org/TR/mediacapture-streams#mediadevices
#[allow(clippy::unused_async)]
pub async fn get_user_media(
    caps: MediaStreamConstraints,
) -> Result<Vec<MediaStreamTrack>, Traced<Error>> {
    unimplemented!()
}

/// Prompts a user to select and grant a permission to capture contents of a
/// display or portion thereof (such as a single window) as vector of
/// [`MediaStreamTrack`].
///
/// Adapter for a [MediaDevices.getDisplayMedia()][1] function.
///
/// # Errors
///
/// With [`MediaManagerError::GetUserMediaFailed`] if
/// [MediaDevices.getDisplayMedia()][1] returns error.
///
/// [1]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
/// [2]: https://w3.org/TR/mediacapture-streams#mediadevices
#[allow(clippy::unused_async)]
pub async fn get_display_media(
    caps: DisplayMediaStreamConstraints,
) -> Result<Vec<MediaStreamTrack>, Traced<Error>> {
    unimplemented!()
}

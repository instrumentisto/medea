use std::convert::TryFrom as _;
use wasm_bindgen_futures::JsFuture;

use tracerr::Traced;

use crate::{
    core::media::MediaManagerError,
    platform::{
        DisplayMediaStreamConstraints, Error, InputDeviceInfo,
        MediaStreamConstraints, MediaStreamTrack,
    },
};

use super::window;

/// Collects information about the User Agent's available media input devices.
///
/// Adapter for [MediaDevices.enumerateDevices()][1].
///
/// # Errors
///
/// With [`MediaManagerError::CouldNotGetMediaDevices`] if could not get
/// [MediaDevices][2].
///
/// With [`MediaManagerError::EnumerateDevicesFailed`] if
/// [MediaDevices.enumerateDevices()][1] returns error.
///
/// [1]: https://tinyurl.com/w3-streams/#dom-mediadevices-enumeratedevices
/// [2]: https://tinyurl.com/w3-streams/#mediadevices
pub async fn enumerate_devices(
) -> Result<Vec<InputDeviceInfo>, Traced<MediaManagerError>> {
    use MediaManagerError::{CouldNotGetMediaDevices, EnumerateDevicesFailed};

    let devices = window()
        .navigator()
        .media_devices()
        .map_err(Error::from)
        .map_err(CouldNotGetMediaDevices)
        .map_err(tracerr::from_and_wrap!())?;
    let devices = JsFuture::from(
        devices
            .enumerate_devices()
            .map_err(Error::from)
            .map_err(EnumerateDevicesFailed)
            .map_err(tracerr::from_and_wrap!())?,
    )
    .await
    .map_err(Error::from)
    .map_err(EnumerateDevicesFailed)
    .map_err(tracerr::from_and_wrap!())?;

    Ok(js_sys::Array::from(&devices)
        .values()
        .into_iter()
        .filter_map(|info| {
            let info = web_sys::MediaDeviceInfo::from(info.unwrap());
            InputDeviceInfo::try_from(info).ok()
        })
        .collect())
}

/// Prompts the user for permission to use a media input which produces vector
/// of [`MediaStreamTrack`]s containing the requested types of media.
///
/// Adapter for [MediaDevices.getUserMedia()][1].
///
/// # Errors
///
/// With [`MediaManagerError::CouldNotGetMediaDevices`] if could not get
/// [MediaDevices][2].
///
/// With [`MediaManagerError::GetUserMediaFailed`] if
/// [MediaDevices.getUserMedia()][1] returns error.
///
/// [1]: https://tinyurl.com/w3-streams/#dom-mediadevices-getusermedia
/// [2]: https://tinyurl.com/w3-streams/#mediadevices
pub async fn get_user_media(
    caps: MediaStreamConstraints,
) -> Result<Vec<MediaStreamTrack>, Traced<MediaManagerError>> {
    use MediaManagerError::{CouldNotGetMediaDevices, GetUserMediaFailed};

    let media_devices = window()
        .navigator()
        .media_devices()
        .map_err(Error::from)
        .map_err(CouldNotGetMediaDevices)
        .map_err(tracerr::from_and_wrap!())?;

    let stream = JsFuture::from(
        media_devices
            .get_user_media_with_constraints(&caps.into())
            .map_err(Error::from)
            .map_err(GetUserMediaFailed)
            .map_err(tracerr::from_and_wrap!())?,
    )
    .await
    .map(web_sys::MediaStream::from)
    .map_err(Error::from)
    .map_err(GetUserMediaFailed)
    .map_err(tracerr::from_and_wrap!())?;

    Ok(js_sys::try_iter(&stream.get_tracks())
        .unwrap()
        .unwrap()
        .map(|tr| MediaStreamTrack::from(tr.unwrap()))
        .collect())
}

/// Prompts the user to select and grant permission to capture the contents of a
/// display or portion thereof (such as a window) as vector of
/// [`MediaStreamTrack`].
///
/// Adapter for [MediaDevices.getDisplayMedia()][1].
///
/// # Errors
///
/// With [`MediaManagerError::CouldNotGetMediaDevices`] if could not get
/// [MediaDevices][2].
///
/// With [`MediaManagerError::GetUserMediaFailed`] if
/// [MediaDevices.getDisplayMedia()][1] returns error.
///
/// [1]: https://www.w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
/// [2]: https://tinyurl.com/w3-streams/#mediadevices
pub async fn get_display_media(
    caps: DisplayMediaStreamConstraints,
) -> Result<Vec<MediaStreamTrack>, Traced<MediaManagerError>> {
    use MediaManagerError::{
        CouldNotGetMediaDevices, GetDisplayMediaFailed, GetUserMediaFailed,
    };

    let media_devices = window()
        .navigator()
        .media_devices()
        .map_err(Error::from)
        .map_err(CouldNotGetMediaDevices)
        .map_err(tracerr::from_and_wrap!())?;

    let stream = JsFuture::from(
        media_devices
            .get_display_media_with_constraints(&caps.into())
            .map_err(Error::from)
            .map_err(GetDisplayMediaFailed)
            .map_err(tracerr::from_and_wrap!())?,
    )
    .await
    .map(web_sys::MediaStream::from)
    .map_err(Error::from)
    .map_err(GetUserMediaFailed)
    .map_err(tracerr::from_and_wrap!())?;

    Ok(js_sys::try_iter(&stream.get_tracks())
        .unwrap()
        .unwrap()
        .map(|tr| MediaStreamTrack::from(tr.unwrap()))
        .collect())
}

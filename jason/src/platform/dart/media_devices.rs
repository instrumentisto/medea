use dart_sys::Dart_Handle;
use tracerr::Traced;

use super::{
    constraints::{DisplayMediaStreamConstraints, MediaStreamConstraints},
    input_device_info::InputDeviceInfo,
    media_track::MediaStreamTrack,
};

use crate::{
    media::MediaManagerError, platform::dart::utils::list::DartList,
    utils::dart::dart_future::DartFuture,
};

type EnumerateDevicesFunction = extern "C" fn() -> Dart_Handle;
static mut ENUMERATE_DEVICES_FUNCTION: Option<EnumerateDevicesFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_MediaDevices__enumerate_devices(
    f: EnumerateDevicesFunction,
) {
    ENUMERATE_DEVICES_FUNCTION = Some(f);
}

pub async fn enumerate_devices(
) -> Result<Vec<InputDeviceInfo>, Traced<MediaManagerError>> {
    let devices =
        DartFuture::new(unsafe { ENUMERATE_DEVICES_FUNCTION.unwrap()() })
            .await
            .unwrap();
    Ok(DartList::from(devices).into())
}

type GetUserMediaFunction = extern "C" fn(Dart_Handle) -> Dart_Handle;
static mut GET_USER_MEDIA_FUNCTION: Option<GetUserMediaFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_MediaDevices__get_user_media(
    f: GetUserMediaFunction,
) {
    GET_USER_MEDIA_FUNCTION = Some(f);
}

pub async fn get_user_media(
    caps: MediaStreamConstraints,
) -> Result<Vec<MediaStreamTrack>, Traced<MediaManagerError>> {
    let tracks = DartFuture::new(unsafe {
        GET_USER_MEDIA_FUNCTION.unwrap()(caps.into())
    })
    .await
    .unwrap();
    Ok(DartList::from(tracks).into())
}

type GetDisplayMediaFunction = extern "C" fn(Dart_Handle) -> Dart_Handle;
static mut GET_DISPLAY_MEDIA_FUNCTION: Option<GetDisplayMediaFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_MediaDevices__get_display_media(
    f: GetUserMediaFunction,
) {
    GET_DISPLAY_MEDIA_FUNCTION = Some(f);
}

pub async fn get_display_media(
    caps: DisplayMediaStreamConstraints,
) -> Result<Vec<MediaStreamTrack>, Traced<MediaManagerError>> {
    let tracks = DartFuture::new(unsafe {
        GET_DISPLAY_MEDIA_FUNCTION.unwrap()(caps.into())
    })
    .await
    .unwrap();
    Ok(DartList::from(tracks).into())
}

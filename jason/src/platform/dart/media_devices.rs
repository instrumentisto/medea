use tracerr::Traced;

use super::{
    constraints::{DisplayMediaStreamConstraints, MediaStreamConstraints},
    input_device_info::InputDeviceInfo,
    media_track::MediaStreamTrack,
};
use crate::media::MediaManagerError;

// type EnumerateDevicesFunction = extern "C" fn() -> Dart_Array;
// static mut enumerate_devices_function: Option<EnumerateDevicesFunction> =
// None;
//
// #[no_mangle]
// pub unsafe extern "C" fn register_MediaDevices__enumerate_devices(f:
// EnumerateDevicesFunction) {     enumerate_devices_function = Some(f);
// }

pub async fn enumerate_devices(
) -> Result<Vec<InputDeviceInfo>, Traced<MediaManagerError>> {
    // let devices = unsafe { enumerate_devices_function.unwrap()() };
    // Ok(devices.into_iter()
    //     .map(|d| InputDeviceInfo::new(d))
    //     .collect())
    todo!()
}

pub async fn get_user_media(
    caps: MediaStreamConstraints,
) -> Result<Vec<MediaStreamTrack>, Traced<MediaManagerError>> {
    todo!()
}

pub async fn get_display_media(
    caps: DisplayMediaStreamConstraints,
) -> Result<Vec<MediaStreamTrack>, Traced<MediaManagerError>> {
    todo!()
}

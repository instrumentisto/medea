use dart_sys::Dart_Handle;
use tracerr::Traced;

use crate::{
    media::MediaManagerError,
    platform::{
        DisplayMediaStreamConstraints, InputDeviceInfo, MediaStreamConstraints,
        MediaStreamTrack,
    },
};

type EnumerateDevicesFunction = extern "C" fn() -> *mut Dart_Handle;
static mut enumerate_devices_function: Option<EnumerateDevicesFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_MediaDevices__enumerate_devices(f: Dart_Handle) {
    enumerate_devices_function = Some(f);
}

pub async fn enumerate_devices(
) -> Result<Vec<InputDeviceInfo>, Traced<MediaManagerError>> {
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

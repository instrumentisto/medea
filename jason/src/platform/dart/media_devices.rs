use dart_sys::Dart_Handle;
use tracerr::Traced;

use crate::{
    media::MediaManagerError,
    platform::{
        DisplayMediaStreamConstraints, InputDeviceInfo, MediaStreamConstraints,
        MediaStreamTrack,
    },
};

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

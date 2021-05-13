use std::ptr::NonNull;

use dart_sys::Dart_Handle;
use tracerr::Traced;

use crate::media::MediaManagerError;

use super::{
    media_stream_settings::MediaStreamSettings,
    utils::{new_handler_detached_error, DartError, IntoDartFuture, PtrArray},
    ForeignClass,
};

#[cfg(feature = "mockable")]
pub use self::mock::MediaManagerHandle;
#[cfg(not(feature = "mockable"))]
pub use crate::media::MediaManagerHandle;

impl ForeignClass for MediaManagerHandle {}

impl From<Traced<MediaManagerError>> for DartError {
    fn from(err: Traced<MediaManagerError>) -> Self {
        let (err, stacktrace) = err.into_parts();
        let stacktrace = stacktrace.to_string();
        match err {
            MediaManagerError::Detached => unsafe {
                new_handler_detached_error(stacktrace)
            },
            MediaManagerError::CouldNotGetMediaDevices(_)
            | MediaManagerError::GetUserMediaFailed(_)
            | MediaManagerError::GetDisplayMediaFailed(_)
            | MediaManagerError::EnumerateDevicesFailed(_)
            | MediaManagerError::LocalTrackIsEnded(_) => {
                todo!()
            }
        }
    }
}

/// Returns [`LocalMediaTrack`]s objects, built from the provided
/// [`MediaStreamSettings`].
///
/// [`LocalMediaTrack`]: crate::media::track::local::LocalMediaTrack
#[no_mangle]
pub unsafe extern "C" fn MediaManagerHandle__init_local_tracks(
    this: NonNull<MediaManagerHandle>,
    caps: NonNull<MediaStreamSettings>,
) -> Dart_Handle {
    let this = this.as_ref().clone();
    let caps = caps.as_ref().clone();

    async move { Ok(PtrArray::new(this.init_local_tracks(caps).await?)) }
        .into_dart_future()
}

/// Returns a list of [`InputDeviceInfo`] objects representing available media
/// input and devices, such as microphones, cameras, and so forth.
///
/// [`InputDeviceInfo`]: super::input_device_info::InputDeviceInfo
#[no_mangle]
pub unsafe extern "C" fn MediaManagerHandle__enumerate_devices(
    this: NonNull<MediaManagerHandle>,
) -> Dart_Handle {
    let this = this.as_ref().clone();

    async move { Ok(PtrArray::new(this.enumerate_devices().await?)) }
        .into_dart_future()
}

/// Frees the data behind the provided pointer.
///
/// # Safety
///
/// Should be called when object is no longer needed. Calling this more than
/// once for the same pointer is equivalent to double free.
#[no_mangle]
pub unsafe extern "C" fn MediaManagerHandle__free(
    this: NonNull<MediaManagerHandle>,
) {
    drop(MediaManagerHandle::from_ptr(this));
}

#[cfg(feature = "mockable")]
mod mock {
    use tracerr::Traced;

    use crate::{
        api::{InputDeviceInfo, LocalMediaTrack, MediaStreamSettings},
        media::MediaManagerError,
    };

    #[derive(Clone)]
    pub struct MediaManagerHandle;

    #[allow(clippy::missing_errors_doc)]
    impl MediaManagerHandle {
        pub async fn enumerate_devices(
            &self,
        ) -> Result<Vec<InputDeviceInfo>, Traced<MediaManagerError>> {
            Ok(vec![
                InputDeviceInfo {},
                InputDeviceInfo {},
                InputDeviceInfo {},
            ])
        }

        pub async fn init_local_tracks(
            &self,
            _caps: MediaStreamSettings,
        ) -> Result<Vec<LocalMediaTrack>, Traced<MediaManagerError>> {
            Ok(vec![
                LocalMediaTrack {},
                LocalMediaTrack {},
                LocalMediaTrack {},
            ])
        }
    }
}

use std::ptr;

use tracerr::Traced;

use crate::media::MediaManagerError;

use super::{
    media_stream_settings::MediaStreamSettings,
    utils::{
        DartError, DartFuture, IntoDartFuture, MediaManagerException,
        MediaManagerExceptionKind, PtrArray, StateError,
    },
    ForeignClass, InputDeviceInfo, LocalMediaTrack,
};

#[cfg(feature = "mockable")]
pub use self::mock::MediaManagerHandle;
#[cfg(not(feature = "mockable"))]
pub use crate::media::MediaManagerHandle;

impl ForeignClass for MediaManagerHandle {}

impl From<Traced<MediaManagerError>> for DartError {
    fn from(err: Traced<MediaManagerError>) -> Self {
        use MediaManagerError as E;
        use MediaManagerExceptionKind as Kind;

        let (err, stacktrace) = err.into_parts();
        let message = err.to_string();

        let (kind, cause) = match err {
            E::GetUserMediaFailed(cause) => {
                (Kind::GetUserMediaFailed, Some(cause))
            }
            E::GetDisplayMediaFailed(cause) => {
                (Kind::GetDisplayMediaFailed, Some(cause))
            }
            E::EnumerateDevicesFailed(cause) => {
                (Kind::EnumerateDevicesFailed, Some(cause))
            }
            E::LocalTrackIsEnded(_) => (Kind::LocalTrackIsEnded, None),
            E::Detached => {
                return StateError::new(
                    "MediaManagerHandle is in detached state.",
                )
                .into()
            }
        };

        MediaManagerException::new(kind, message, cause, stacktrace).into()
    }
}

/// Returns [`LocalMediaTrack`]s objects, built from the provided
/// [`MediaStreamSettings`].
///
/// [`LocalMediaTrack`]: crate::media::track::local::LocalMediaTrack
#[no_mangle]
pub unsafe extern "C" fn MediaManagerHandle__init_local_tracks(
    this: ptr::NonNull<MediaManagerHandle>,
    caps: ptr::NonNull<MediaStreamSettings>,
) -> DartFuture<Result<PtrArray<LocalMediaTrack>, Traced<MediaManagerError>>> {
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
    this: ptr::NonNull<MediaManagerHandle>,
) -> DartFuture<Result<PtrArray<InputDeviceInfo>, Traced<MediaManagerError>>> {
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
    this: ptr::NonNull<MediaManagerHandle>,
) {
    drop(MediaManagerHandle::from_ptr(this));
}

#[cfg(feature = "mockable")]
mod mock {
    use dart_sys::Dart_Handle;
    use tracerr::Traced;

    use crate::{
        api::{
            dart::{
                utils::{DartFuture, DartResult, IntoDartFuture},
                DartError,
            },
            InputDeviceInfo, LocalMediaTrack, MediaStreamSettings,
        },
        media::MediaManagerError,
        platform,
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

    #[no_mangle]
    pub unsafe extern "C" fn returns_media_manager_exception(
        cause: Dart_Handle,
    ) -> DartResult {
        let err = tracerr::new!(MediaManagerError::GetUserMediaFailed(
            platform::Error::from(cause)
        ));
        DartError::from(err).into()
    }

    #[no_mangle]
    pub unsafe extern "C" fn returns_future_with_media_manager_exception(
        cause: Dart_Handle,
    ) -> DartFuture<Result<(), Traced<MediaManagerError>>> {
        let cause = platform::Error::from(cause);
        let err =
            tracerr::new!(MediaManagerError::GetDisplayMediaFailed(cause));

        async move { Result::<(), _>::Err(err) }.into_dart_future()
    }
}

use std::ptr;

use dart_sys::Dart_Handle;

use super::{
    media_stream_settings::MediaStreamSettings,
    utils::{IntoDartFuture, PtrArray},
    ForeignClass,
};

#[cfg(feature = "mockable")]
pub use self::mock::MediaManagerHandle;
#[cfg(not(feature = "mockable"))]
pub use crate::media::MediaManagerHandle;

impl ForeignClass for MediaManagerHandle {}

/// Returns [`LocalMediaTrack`]s objects, built from the provided
/// [`MediaStreamSettings`].
///
/// [`LocalMediaTrack`]: crate::media::track::local::LocalMediaTrack
#[no_mangle]
pub unsafe extern "C" fn MediaManagerHandle__init_local_tracks(
    this: ptr::NonNull<MediaManagerHandle>,
    caps: ptr::NonNull<MediaStreamSettings>,
) -> Dart_Handle {
    let this = this.as_ref().clone();
    let caps = caps.as_ref().clone();

    async move {
        // TODO: Remove unwrap when propagating errors from Rust to Dart is
        //       implemented.
        Ok::<_, ()>(PtrArray::new(this.init_local_tracks(caps).await.unwrap()))
    }
    .into_dart_future()
}

/// Returns a list of [`InputDeviceInfo`] objects representing available media
/// input and devices, such as microphones, cameras, and so forth.
///
/// [`InputDeviceInfo`]: super::input_device_info::InputDeviceInfo
#[no_mangle]
pub unsafe extern "C" fn MediaManagerHandle__enumerate_devices(
    this: ptr::NonNull<MediaManagerHandle>,
) -> Dart_Handle {
    let this = this.as_ref().clone();

    async move {
        // TODO: Remove unwrap when propagating errors from Rust to Dart is
        //       implemented.
        Ok::<_, ()>(PtrArray::new(this.enumerate_devices().await.unwrap()))
    }
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
    use crate::api::{
        InputDeviceInfo, JasonError, LocalMediaTrack, MediaStreamSettings,
    };

    #[derive(Clone)]
    pub struct MediaManagerHandle;

    #[allow(clippy::missing_errors_doc)]
    impl MediaManagerHandle {
        pub async fn enumerate_devices(
            &self,
        ) -> Result<Vec<InputDeviceInfo>, JasonError> {
            Ok(vec![
                InputDeviceInfo {},
                InputDeviceInfo {},
                InputDeviceInfo {},
            ])
        }

        pub async fn init_local_tracks(
            &self,
            _caps: MediaStreamSettings,
        ) -> Result<Vec<LocalMediaTrack>, JasonError> {
            Ok(vec![
                LocalMediaTrack {},
                LocalMediaTrack {},
                LocalMediaTrack {},
            ])
        }
    }
}

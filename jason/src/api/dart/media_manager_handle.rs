use futures::FutureExt as _;

use super::{
    input_device_info::InputDeviceInfo, local_media_track::LocalMediaTrack,
    media_stream_settings::MediaStreamSettings, utils::PtrArray, ForeignClass,
};

#[cfg(feature = "mockable")]
pub use self::mock::MediaManagerHandle;
#[cfg(not(feature = "mockable"))]
pub use crate::media::MediaManagerHandle;

impl ForeignClass for MediaManagerHandle {}

/// Returns [`LocalMediaTrack`]s objects, built from the provided
/// [`MediaStreamSettings`].
#[no_mangle]
pub unsafe extern "C" fn MediaManagerHandle__init_local_tracks(
    this: *const MediaManagerHandle,
    caps: *const MediaStreamSettings,
) -> PtrArray<LocalMediaTrack> {
    let this = this.as_ref().unwrap();
    let caps = caps.as_ref().unwrap();

    // TODO: Remove now_or_never when polling from Dart is implemented.
    //       Remove unwrap when propagating errors from Rust to Dart is
    //       implemented.
    PtrArray::new(
        this.init_local_tracks(caps.clone())
            .now_or_never()
            .unwrap()
            .unwrap(),
    )
}

/// Returns a list of [`InputDeviceInfo`] objects representing available media
/// input and devices, such as microphones, cameras, and so forth.
#[no_mangle]
pub unsafe extern "C" fn MediaManagerHandle__enumerate_devices(
    this: *const MediaManagerHandle,
) -> PtrArray<InputDeviceInfo> {
    let this = this.as_ref().unwrap();

    // TODO: Remove now_or_never when polling from Dart is implemented.
    //       Remove unwrap when propagating errors from Rust to Dart is
    //       implemented.
    PtrArray::new(this.enumerate_devices().now_or_never().unwrap().unwrap())
}

/// Frees the data behind the provided pointer.
///
/// # Safety
///
/// Should be called when object is no longer needed. Calling this more than
/// once for the same pointer is equivalent to double free.
#[no_mangle]
pub unsafe extern "C" fn MediaManagerHandle__free(
    this: *mut MediaManagerHandle,
) {
    drop(MediaManagerHandle::from_ptr(this));
}

#[cfg(feature = "mockable")]
mod mock {
    use crate::api::{
        InputDeviceInfo, JasonError, LocalMediaTrack, MediaStreamSettings,
    };

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

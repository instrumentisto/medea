use std::ptr::NonNull;

use super::ForeignClass;

#[cfg(feature = "mockable")]
pub use self::mock::LocalMediaTrack;
#[cfg(not(feature = "mockable"))]
pub use crate::media::track::local::LocalMediaTrack;

impl ForeignClass for LocalMediaTrack {}

/// Returns a [`MediaKind::Audio`] if this [`LocalMediaTrack`] represents an
/// audio track, or a [`MediaKind::Video`] if it represents a video track.
///
/// [`MediaKind::Audio`]: crate::media::MediaKind::Audio
/// [`MediaKind::Video`]: crate::media::MediaKind::Video
#[no_mangle]
pub unsafe extern "C" fn LocalMediaTrack__kind(
    this: NonNull<LocalMediaTrack>,
) -> u8 {
    let this = this.as_ref();

    this.kind() as u8
}

/// Returns a [`MediaSourceKind::Device`] if this [`LocalMediaTrack`] is
/// sourced from some device (webcam/microphone), or a
/// [`MediaSourceKind::Display`] if it's captured via
/// [MediaDevices.getDisplayMedia()][1].
///
/// [1]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
/// [`MediaSourceKind::Device`]: crate::media::MediaSourceKind::Device
/// [`MediaSourceKind::Display`]: crate::media::MediaSourceKind::Display
#[no_mangle]
pub unsafe extern "C" fn LocalMediaTrack__media_source_kind(
    this: NonNull<LocalMediaTrack>,
) -> u8 {
    let this = this.as_ref();

    this.media_source_kind() as u8
}

/// Frees the data behind the provided pointer.
///
/// # Safety
///
/// Should be called when object is no longer needed. Calling this more than
/// once for the same pointer is equivalent to double free.
#[no_mangle]
pub unsafe extern "C" fn LocalMediaTrack__free(this: NonNull<LocalMediaTrack>) {
    drop(LocalMediaTrack::from_ptr(this));
}

#[cfg(feature = "mockable")]
mod mock {
    use crate::media::{MediaKind, MediaSourceKind};

    pub struct LocalMediaTrack;

    impl LocalMediaTrack {
        pub fn kind(&self) -> MediaKind {
            MediaKind::Video
        }

        pub fn media_source_kind(&self) -> MediaSourceKind {
            MediaSourceKind::Display
        }

        // pub fn get_track(&self) -> sys::MediaStreamTrack
    }
}

use std::ptr;

use dart_sys::Dart_Handle;

use crate::{
    media::{MediaKind, MediaSourceKind},
    platform,
};

use super::ForeignClass;

#[cfg(feature = "mockable")]
pub use self::mock::RemoteMediaTrack;
#[cfg(not(feature = "mockable"))]
pub use crate::media::track::remote::Track as RemoteMediaTrack;

impl ForeignClass for RemoteMediaTrack {}

/// Sets callback, invoked when this [`RemoteMediaTrack`] is enabled.
#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__on_enabled(
    this: ptr::NonNull<RemoteMediaTrack>,
    f: Dart_Handle,
) {
    this.as_ref().on_enabled(platform::Function::new(f));
}

/// Sets callback, invoked when this [`RemoteMediaTrack`] is disabled.
#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__on_disabled(
    this: ptr::NonNull<RemoteMediaTrack>,
    f: Dart_Handle,
) {
    this.as_ref().on_disabled(platform::Function::new(f));
}

/// Sets callback to invoke when this [`RemoteMediaTrack`] is muted.
#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__on_muted(
    this: ptr::NonNull<RemoteMediaTrack>,
    f: Dart_Handle,
) {
    this.as_ref().on_muted(platform::Function::new(f));
}

/// Sets callback to invoke when this [`RemoteMediaTrack`] is unmuted.
#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__on_unmuted(
    this: ptr::NonNull<RemoteMediaTrack>,
    f: Dart_Handle,
) {
    this.as_ref().on_unmuted(platform::Function::new(f));
}

/// Sets callback to invoke when this [`RemoteMediaTrack`] is stopped.
#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__on_stopped(
    this: ptr::NonNull<RemoteMediaTrack>,
    f: Dart_Handle,
) {
    this.as_ref().on_stopped(platform::Function::new(f));
}

/// Indicates whether this [`RemoteMediaTrack`] is enabled.
#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__enabled(
    this: ptr::NonNull<RemoteMediaTrack>,
) -> u8 {
    this.as_ref().enabled() as u8
}

/// Indicate whether this [`RemoteMediaTrack`] is muted.
#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__muted(
    this: ptr::NonNull<RemoteMediaTrack>,
) -> u8 {
    this.as_ref().muted() as u8
}

/// Returns this [`RemoteMediaTrack`]'s kind (audio/video).
#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__kind(
    this: ptr::NonNull<RemoteMediaTrack>,
) -> MediaKind {
    this.as_ref().kind()
}

/// Returns this [`RemoteMediaTrack`]'s media source kind.
#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__media_source_kind(
    this: ptr::NonNull<RemoteMediaTrack>,
) -> MediaSourceKind {
    this.as_ref().media_source_kind()
}

/// Frees the data behind the provided pointer.
///
/// # Safety
///
/// Should be called when object is no longer needed. Calling this more than
/// once for the same pointer is equivalent to double free.
#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__free(
    this: ptr::NonNull<RemoteMediaTrack>,
) {
    drop(RemoteMediaTrack::from_ptr(this));
}

#[cfg(feature = "mockable")]
mod mock {
    use crate::{
        media::{
            track::remote::Track as CoreRemoteMediaTrack, MediaKind,
            MediaSourceKind,
        },
        platform,
    };

    pub struct RemoteMediaTrack;

    impl From<CoreRemoteMediaTrack> for RemoteMediaTrack {
        fn from(_: CoreRemoteMediaTrack) -> Self {
            Self
        }
    }

    impl RemoteMediaTrack {
        pub fn enabled(&self) -> bool {
            true
        }

        pub fn kind(&self) -> MediaKind {
            MediaKind::Video
        }

        pub fn media_source_kind(&self) -> MediaSourceKind {
            MediaSourceKind::Device
        }

        pub fn muted(&self) -> bool {
            false
        }

        // pub fn get_track(&self) -> sys::MediaStreamTrack

        pub fn on_enabled(&self, cb: platform::Function<()>) {
            cb.call0();
        }

        pub fn on_disabled(&self, cb: platform::Function<()>) {
            cb.call0();
        }

        pub fn on_muted(&self, cb: platform::Function<()>) {
            cb.call0();
        }

        pub fn on_unmuted(&self, cb: platform::Function<()>) {
            cb.call0();
        }

        pub fn on_stopped(&self, cb: platform::Function<()>) {
            cb.call0();
        }
    }
}

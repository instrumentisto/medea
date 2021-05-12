use std::ptr::NonNull;

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
    this: NonNull<RemoteMediaTrack>,
    f: Dart_Handle,
) {
    let this = this.as_ref();
    this.on_enabled(platform::Function::new(f));
}

/// Sets callback, invoked when this [`RemoteMediaTrack`] is disabled.
#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__on_disabled(
    this: NonNull<RemoteMediaTrack>,
    f: Dart_Handle,
) {
    let this = this.as_ref();
    this.on_disabled(platform::Function::new(f));
}

/// Sets callback to invoke when this [`RemoteMediaTrack`] is muted.
#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__on_muted(
    this: NonNull<RemoteMediaTrack>,
    f: Dart_Handle,
) {
    let this = this.as_ref();
    this.on_muted(platform::Function::new(f));
}

/// Sets callback to invoke when this [`RemoteMediaTrack`] is unmuted.
#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__on_unmuted(
    this: NonNull<RemoteMediaTrack>,
    f: Dart_Handle,
) {
    let this = this.as_ref();
    this.on_unmuted(platform::Function::new(f));
}

/// Sets callback to invoke when this [`RemoteMediaTrack`] is stopped.
#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__on_stopped(
    this: NonNull<RemoteMediaTrack>,
    f: Dart_Handle,
) {
    let this = this.as_ref();
    this.on_stopped(platform::Function::new(f));
}

/// Indicates whether this [`RemoteMediaTrack`] is enabled.
#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__enabled(
    this: NonNull<RemoteMediaTrack>,
) -> u8 {
    let this = this.as_ref();

    this.enabled() as u8
}

/// Indicate whether this [`RemoteMediaTrack`] is muted.
#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__muted(
    this: NonNull<RemoteMediaTrack>,
) -> u8 {
    let this = this.as_ref();

    this.muted() as u8
}

/// Returns this [`RemoteMediaTrack`]'s kind (audio/video).
#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__kind(
    this: NonNull<RemoteMediaTrack>,
) -> MediaKind {
    let this = this.as_ref();

    this.kind()
}

/// Returns this [`RemoteMediaTrack`]'s media source kind.
#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__media_source_kind(
    this: NonNull<RemoteMediaTrack>,
) -> MediaSourceKind {
    let this = this.as_ref();

    this.media_source_kind()
}

/// Frees the data behind the provided pointer.
///
/// # Safety
///
/// Should be called when object is no longer needed. Calling this more than
/// once for the same pointer is equivalent to double free.
#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__free(
    this: NonNull<RemoteMediaTrack>,
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

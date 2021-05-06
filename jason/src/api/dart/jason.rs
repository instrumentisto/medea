use std::ptr::NonNull;

use super::{
    media_manager_handle::MediaManagerHandle, room_handle::RoomHandle,
    ForeignClass,
};

#[cfg(feature = "mockable")]
pub use self::mock::Jason;
#[cfg(not(feature = "mockable"))]
pub use crate::jason::Jason;

impl ForeignClass for Jason {}

/// Instantiates a new [`Jason`] interface to interact with this library.
#[no_mangle]
pub extern "C" fn Jason__new() -> NonNull<Jason> {
    Jason::new().into_ptr()
}

/// Creates a new [`Room`] and returns its [`RoomHandle`].
///
/// [`Room`]: crate::room::Room
#[no_mangle]
pub unsafe extern "C" fn Jason__init_room(
    this: NonNull<Jason>,
) -> NonNull<RoomHandle> {
    let this = this.as_ref();

    this.init_room().into_ptr()
}

/// Returns a [`MediaManagerHandle`].
#[no_mangle]
pub unsafe extern "C" fn Jason__media_manager(
    this: NonNull<Jason>,
) -> NonNull<MediaManagerHandle> {
    let this = this.as_ref();

    this.media_manager().into_ptr()
}

/// Closes the provided [`RoomHandle`].
#[no_mangle]
pub unsafe extern "C" fn Jason__close_room(
    this: NonNull<Jason>,
    room_to_delete: NonNull<RoomHandle>,
) {
    let this = this.as_ref();

    this.close_room(RoomHandle::from_ptr(room_to_delete));
}

/// Frees the data behind the provided pointer.
///
/// # Safety
///
/// Should be called when object is no longer needed. Calling this more than
/// once for the same pointer is equivalent to double free.
#[no_mangle]
pub unsafe extern "C" fn Jason__free(this: NonNull<Jason>) {
    let _ = Jason::from_ptr(this);
}

#[cfg(feature = "mockable")]
mod mock {
    use crate::api::{MediaManagerHandle, RoomHandle};

    pub struct Jason;

    impl Jason {
        pub fn new() -> Self {
            crate::platform::init_logger();
            Self
        }

        pub fn init_room(&self) -> RoomHandle {
            RoomHandle
        }

        pub fn media_manager(&self) -> MediaManagerHandle {
            MediaManagerHandle
        }

        pub fn close_room(&self, _: RoomHandle) {}
    }
}

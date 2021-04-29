use std::ptr::NonNull;

use crate::{
    media_manager::MediaManagerHandle, room_handle::RoomHandle, ForeignClass,
};

pub struct Jason;

impl ForeignClass for Jason {}

impl Jason {
    pub fn new() -> Self {
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

#[no_mangle]
pub extern "C" fn Jason__new() -> *const Jason {
    Jason::new().into_ptr()
}

#[no_mangle]
pub unsafe extern "C" fn Jason__init_room(
    this: NonNull<Jason>,
) -> *const RoomHandle {
    let this = this.as_ref();

    this.init_room().into_ptr()
}

#[no_mangle]
pub unsafe extern "C" fn Jason__media_manager(
    this: NonNull<Jason>,
) -> *const MediaManagerHandle {
    let this = this.as_ref();

    this.media_manager().into_ptr()
}

#[no_mangle]
pub unsafe extern "C" fn Jason__close_room(
    this: NonNull<Jason>,
    room_to_delete: NonNull<RoomHandle>,
) {
    let this = this.as_ref();

    this.close_room(RoomHandle::from_ptr(room_to_delete));
}

#[no_mangle]
pub unsafe extern "C" fn Jason__free(this: NonNull<Jason>) {
    Jason::from_ptr(this);
}

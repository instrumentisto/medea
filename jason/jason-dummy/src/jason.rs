use crate::{
    media_manager::MediaManagerHandle, room_handle::RoomHandle,
    utils::ptr_from_dart_as_mut,
};

pub struct Jason;

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
    Box::into_raw(Box::new(Jason::new()))
}

#[no_mangle]
pub unsafe extern "C" fn Jason__init_room(this: *mut Jason) -> *mut RoomHandle {
    Box::into_raw(Box::new(ptr_from_dart_as_mut(this).init_room()))
}

#[no_mangle]
pub unsafe extern "C" fn Jason__media_manager(
    this: *mut Jason,
) -> *mut MediaManagerHandle {
    Box::into_raw(Box::new(ptr_from_dart_as_mut(this).media_manager()))
}

#[no_mangle]
pub unsafe extern "C" fn Jason__close_room(
    this: *mut Jason,
    room_to_delete: *mut RoomHandle,
) {
    ptr_from_dart_as_mut(this).close_room(*Box::from_raw(room_to_delete));
}

#[no_mangle]
pub unsafe extern "C" fn Jason__free(this: *mut Jason) {
    Box::from_raw(this);
}

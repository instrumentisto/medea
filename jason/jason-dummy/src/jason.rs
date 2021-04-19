use crate::{
    media_manager::MediaManagerHandle, room_handle::RoomHandle,
    utils::ptr_from_dart_as_ref,
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
pub unsafe extern "C" fn Jason__init_room(
    this: *const Jason,
) -> *mut RoomHandle {
    let this = ptr_from_dart_as_ref(this);

    Box::into_raw(Box::new(this.init_room()))
}

#[no_mangle]
pub unsafe extern "C" fn Jason__media_manager(
    this: *const Jason,
) -> *mut MediaManagerHandle {
    let this = ptr_from_dart_as_ref(this);

    Box::into_raw(Box::new(this.media_manager()))
}

#[no_mangle]
pub unsafe extern "C" fn Jason__close_room(
    this: *const Jason,
    room_to_delete: *mut RoomHandle,
) {
    let this = ptr_from_dart_as_ref(this);

    this.close_room(*Box::from_raw(room_to_delete));
}

#[no_mangle]
pub unsafe extern "C" fn Jason__free(this: *mut Jason) {
    if !this.is_null() {
        Box::from_raw(this);
    }
}

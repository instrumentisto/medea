use crate::{media_manager::MediaManager, room_handle::RoomHandle};

pub struct Jason;

impl Jason {
    pub fn init_room(&self) -> RoomHandle {
        RoomHandle
    }

    pub fn media_manager(&self) -> MediaManager {
        MediaManager
    }

    pub fn foobar(&self) -> String {
        "foobar".to_string()
    }

    pub fn close_room(&self, room_to_delete: &RoomHandle) {}
}

#[no_mangle]
pub unsafe extern "C" fn Jason__init_room(this: *mut Jason) -> *mut RoomHandle {
    let this = Box::from_raw(this);
    Box::into_raw(Box::new(this.init_room()))
}

#[no_mangle]
pub unsafe extern "C" fn Jason__media_manager(
    this: *mut Jason,
) -> *mut MediaManager {
    let this = Box::from_raw(this);
    Box::into_raw(Box::new(this.media_manager()))
}

#[no_mangle]
pub unsafe extern "C" fn Jason__close_room(
    this: *mut Jason,
    room_to_delete: *mut RoomHandle,
) {
    let this = Box::from_raw(this);
    let room_to_delete = Box::from_raw(room_to_delete);
    this.close_room(&room_to_delete);
}

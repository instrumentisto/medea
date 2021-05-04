use crate::{
    media_manager_handle::MediaManagerHandle, room_handle::RoomHandle,
    ForeignClass,
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
    // TODO: init_once should be called only once.
    android_logger::init_once(
        android_logger::Config::default().with_min_level(log::Level::Debug),
    );
    Jason::new().into_ptr()
}

#[no_mangle]
pub unsafe extern "C" fn Jason__init_room(
    this: *const Jason,
) -> *const RoomHandle {
    let this = this.as_ref().unwrap();

    this.init_room().into_ptr()
}

#[no_mangle]
pub unsafe extern "C" fn Jason__media_manager(
    this: *const Jason,
) -> *const MediaManagerHandle {
    let this = this.as_ref().unwrap();

    this.media_manager().into_ptr()
}

#[no_mangle]
pub unsafe extern "C" fn Jason__close_room(
    this: *const Jason,
    room_to_delete: *mut RoomHandle,
) {
    let this = this.as_ref().unwrap();

    this.close_room(RoomHandle::from_ptr(room_to_delete));
}

#[no_mangle]
pub unsafe extern "C" fn Jason__free(this: *mut Jason) {
    Jason::from_ptr(this);
}

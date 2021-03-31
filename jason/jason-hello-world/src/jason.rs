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

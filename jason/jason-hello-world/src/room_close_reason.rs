use crate::into_dart_string;

pub struct RoomCloseReason;

impl RoomCloseReason {
    pub fn reason(&self) -> String {
        "RoomClose reason string".to_string()
    }

    pub fn is_closed_by_server(&self) -> bool {
        false
    }

    pub fn is_err(&self) -> bool {
        false
    }
}

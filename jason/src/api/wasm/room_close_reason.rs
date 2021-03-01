use derive_more::From;
use wasm_bindgen::prelude::*;

use crate::room;

/// Reason of why [`Room`] has been closed.
///
/// This struct is passed into `on_close_by_server` JS side callback.
///
/// [`Room`]: room::Room
#[wasm_bindgen]
#[derive(From)]
pub struct RoomCloseReason(room::RoomCloseReason);

#[wasm_bindgen]
impl RoomCloseReason {
    /// [`Room`] close reason.
    ///
    /// [`Room`]: room::Room
    pub fn reason(&self) -> String {
        self.0.reason()
    }

    /// Whether [`Room`] was closed by server.
    ///
    /// [`Room`]: room::Room
    pub fn is_closed_by_server(&self) -> bool {
        self.0.is_closed_by_server()
    }

    /// Whether [`Room`] close reason is considered as error
    ///
    /// [`Room`]: room::Room
    pub fn is_err(&self) -> bool {
        self.0.is_err()
    }
}

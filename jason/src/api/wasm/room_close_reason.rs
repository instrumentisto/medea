use derive_more::From;
use wasm_bindgen::prelude::*;

use crate::core;

/// Reason of why [`Room`] has been closed.
///
/// This struct is passed into `on_close_by_server` JS side callback.
///
/// [`Room`]: core::Room
#[wasm_bindgen]
#[derive(From)]
pub struct RoomCloseReason(core::RoomCloseReason);

#[wasm_bindgen]
impl RoomCloseReason {
    /// [`Room`] close reason.
    ///
    /// [`Room`]: core::Room
    pub fn reason(&self) -> String {
        self.0.reason()
    }

    /// Whether [`Room`] was closed by server.
    ///
    /// [`Room`]: core::Room
    pub fn is_closed_by_server(&self) -> bool {
        self.0.is_closed_by_server()
    }

    /// Whether [`Room`] close reason is considered as error
    ///
    /// [`Room`]: core::Room
    pub fn is_err(&self) -> bool {
        self.0.is_err()
    }
}

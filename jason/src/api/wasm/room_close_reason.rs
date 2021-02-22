use derive_more::From;
use wasm_bindgen::prelude::*;

use crate::core;

/// Reason of why [`Room`] has been closed.
///
/// This struct is passed into `on_close_by_server` JS side callback.
#[wasm_bindgen]
#[derive(From)]
pub struct RoomCloseReason(core::RoomCloseReason);

#[wasm_bindgen]
impl RoomCloseReason {
    /// `wasm_bindgen` getter for [`RoomCloseReason::reason`] field.
    pub fn reason(&self) -> String {
        self.0.reason()
    }

    /// `wasm_bindgen` getter for [`RoomCloseReason::is_closed_by_server`]
    /// field.
    pub fn is_closed_by_server(&self) -> bool {
        self.0.is_closed_by_server()
    }

    /// `wasm_bindgen` getter for [`RoomCloseReason::is_err`] field.
    pub fn is_err(&self) -> bool {
        self.0.is_err()
    }
}

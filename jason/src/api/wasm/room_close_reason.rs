//! Reason of a [`Room`] closing.
//!
//! [`Room`]: room::Room

use derive_more::From;
use wasm_bindgen::prelude::*;

use crate::room;

/// Reason of why a [`Room`] is closed.
///
/// This struct is passed to a [`RoomHandle::on_close`] JS side callback.
///
/// [`Room`]: room::Room
/// [`RoomHandle::on_close`]: crate::api::RoomHandle::on_close
#[wasm_bindgen]
#[derive(From)]
pub struct RoomCloseReason(room::RoomCloseReason);

#[wasm_bindgen]
impl RoomCloseReason {
    /// Returns the [`Room`]'s close reason.
    ///
    /// [`Room`]: room::Room
    #[must_use]
    pub fn reason(&self) -> String {
        self.0.reason()
    }

    /// Indicates whether the [`Room`] was closed by server.
    ///
    /// [`Room`]: room::Room
    #[must_use]
    pub fn is_closed_by_server(&self) -> bool {
        self.0.is_closed_by_server()
    }

    /// Indicates whether the [`Room`] close reason is considered as an error.
    ///
    /// [`Room`]: room::Room
    #[must_use]
    pub fn is_err(&self) -> bool {
        self.0.is_err()
    }
}

//! Exception returned from [`RoomHandle::set_local_media_settings()`][1].
//!
//! [1]: crate::api::RoomHandle::set_local_media_settings

use derive_more::From;
use wasm_bindgen::prelude::*;

use crate::room;

/// Exception returned from [`RoomHandle::set_local_media_settings()`][1].
///
/// [1]: crate::api::RoomHandle::set_local_media_settings
#[wasm_bindgen]
#[derive(Debug, From)]
#[from(forward)]
pub struct ConstraintsUpdateException(room::ConstraintsUpdateError);

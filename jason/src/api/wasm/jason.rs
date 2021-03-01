use derive_more::From;
use wasm_bindgen::prelude::*;

use crate::{
    api::{MediaManagerHandle, RoomHandle},
    jason,
    platform::{init_logger, set_panic_hook},
};

/// General library interface.
///
/// Responsible for managing shared transports, local media
/// and room initialization.
#[wasm_bindgen]
#[derive(From)]
pub struct Jason(jason::Jason);

#[wasm_bindgen]
impl Jason {
    /// Instantiates new [`Jason`] interface to interact with this library.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        set_panic_hook();
        init_logger();

        Self(jason::Jason::new())
    }

    /// Creates new `Room` and returns its [`RoomHandle`].
    pub fn init_room(&self) -> RoomHandle {
        self.0.init_room().into()
    }

    /// Returns [`MediaManagerHandle`].
    pub fn media_manager(&self) -> MediaManagerHandle {
        self.0.media_manager().into()
    }

    /// Closes the provided [`RoomHandle`].
    #[allow(clippy::needless_pass_by_value)]
    pub fn close_room(&self, room_to_delete: RoomHandle) {
        self.0.close_room(room_to_delete.into());
    }

    /// Drops [`Jason`] API object, so all related objects (rooms, connections,
    /// streams etc.) respectively. All objects related to this [`Jason`] API
    /// object will be detached (you will still hold them, but unable to use).
    pub fn dispose(self) {
        self.0.dispose();
    }
}

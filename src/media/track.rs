//!  Remote [`MediaStreamTrack`][1] representation.
//!
//! [1]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack

use std::{cell::RefCell, sync::Mutex};

use medea_client_api_proto::MediaType;

/// ID of [`Track`].
pub type Id = u64;

/// [`MediaStreamTrack`] representation.
#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
pub struct MediaTrack {
    pub id: Id,
    mid: Mutex<RefCell<Option<String>>>,
    pub media_type: MediaType,
}

impl MediaTrack {
    /// Creates new [`Track`] of the specified type.
    pub fn new(id: Id, media_type: MediaType) -> Self {
        Self {
            id,
            mid: Mutex::new(RefCell::new(None)),
            media_type,
        }
    }

    pub fn set_mid(&self, mid: String) {
        self.mid.lock().unwrap().borrow_mut().replace(mid);
    }

    pub fn mid(&self) -> Option<String> {
        self.mid.lock().unwrap().borrow().as_ref().cloned()
    }
}

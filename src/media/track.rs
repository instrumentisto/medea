//! Remote [MediaStreamTrack][1] representation.
//!
//! [1]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack

use std::cell::RefCell;

use medea_client_api_proto::{MediaType, TrackId as Id, TrackPatch};

/// Representation of [MediaStreamTrack][1] object.
///
/// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack
#[derive(Debug)]
pub struct MediaTrack {
    pub id: Id,
    mid: RefCell<Option<String>>,
    pub media_type: MediaType,
    is_muted: RefCell<bool>,
}

impl MediaTrack {
    /// Creates new [`MediaTrack`] of the specified [`MediaType`].
    pub fn new(id: Id, media_type: MediaType) -> Self {
        Self {
            id,
            mid: RefCell::new(None),
            media_type,
            is_muted: RefCell::new(false),
        }
    }

    pub fn set_mid(&self, mid: String) {
        self.mid.borrow_mut().replace(mid);
    }

    pub fn mid(&self) -> Option<String> {
        self.mid.borrow_mut().as_ref().cloned()
    }

    pub fn update(&self, patch: &TrackPatch) {
        if let Some(is_muted) = patch.is_muted {
            *self.is_muted.borrow_mut() = is_muted;
        }
    }

    pub fn is_muted(&self) -> bool {
        *self.is_muted.borrow()
    }
}

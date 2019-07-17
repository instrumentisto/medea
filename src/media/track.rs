//! Remote [MediaStreamTrack][1] representation.
//!
//! [1]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack

use std::cell::RefCell;

use medea_client_api_proto::MediaType;

/// ID of [`MediaTrack`].
pub type Id = u64;

/// Representation of [MediaStreamTrack][1] object.
///
/// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack
#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
pub struct MediaTrack {
    pub id: Id,
    mid: RefCell<Option<String>>,
    pub media_type: MediaType,
}

impl MediaTrack {
    /// Creates new [`MediaTrack`] of the specified [`MediaType`].
    pub fn new(id: Id, media_type: MediaType) -> Self {
        Self {
            id,
            mid: RefCell::new(None),
            media_type,
        }
    }

    pub fn set_mid(&self, mid: String) {
        self.mid.borrow_mut().replace(mid);
    }

    pub fn mid(&self) -> Option<String> {
        self.mid.borrow_mut().as_ref().cloned()
    }
}

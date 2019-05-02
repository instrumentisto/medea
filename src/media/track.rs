//!  Remote [MediaStreamTrack] representation.
//!
//! [MediaStreamTrack]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack

use crate::api::protocol::MediaType;

/// ID of [`Track`].
pub type Id = u64;

/// [`MediaStreamTrack`] representation.
#[derive(Debug)]
pub struct Track {
    pub id: Id,
    pub media_type: MediaType,
}

impl Track {
    /// Creates new [`Track`] of the specified type.
    pub fn new(id: Id, media_type: MediaType) -> Self {
        Self { id, media_type }
    }
}

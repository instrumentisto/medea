//!  Remote [`MediaStreamTrack`][1] representation.
//!
//! [1]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack

use medea_client_api_proto::MediaType;

/// ID of [`Track`].
pub type Id = u64;

/// [`MediaStreamTrack`] representation.
#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
pub struct MediaTrack {
    pub id: Id,
    pub media_type: MediaType,
}

impl MediaTrack {
    /// Creates new [`Track`] of the specified type.
    pub fn new(id: Id, media_type: MediaType) -> Self {
        Self { id, media_type }
    }
}

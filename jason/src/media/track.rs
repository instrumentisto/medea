//! [MediaStreamTrack][1] representation.
//!
//! [1]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack

use std::rc::Rc;

use medea_client_api_proto::MediaType;
use web_sys::MediaStreamTrack;

/// ID of [`MediaTrack`].
pub type Id = u64;

/// Representation of [MediaStreamTrack][1].
///
/// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack
#[allow(clippy::module_name_repetitions)]
pub struct MediaTrack {
    id: Id,
    track: MediaStreamTrack,
    caps: MediaType,
}

impl MediaTrack {
    /// Instantiates new [`MediaTrack`].
    pub fn new(id: u64, track: MediaStreamTrack, caps: MediaType) -> Rc<Self> {
        Rc::new(Self { id, track, caps })
    }

    /// Returns ID of this [`MediaTrack`].
    pub fn id(&self) -> Id {
        self.id
    }

    /// Returns the underlying [`MediaStreamTrack`] object of this
    /// [`MediaTrack`].
    pub fn track(&self) -> &MediaStreamTrack {
        &self.track
    }

    /// Returns [`MediaType`] of this [`MediaTrack`].
    pub fn caps(&self) -> &MediaType {
        &self.caps
    }
}

impl Drop for MediaTrack {
    fn drop(&mut self) {
        self.track.stop()
    }
}
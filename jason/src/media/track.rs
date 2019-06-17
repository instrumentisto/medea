//! [`MediaStreamTrack`][1] wrapper.
//!
//! [1]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack
use std::rc::Rc;

use medea_client_api_proto::MediaType;
use web_sys::MediaStreamTrack;

pub type Id = u64;

#[allow(clippy::module_name_repetitions)]
pub struct MediaTrack {
    id: Id,
    track: MediaStreamTrack,
    caps: MediaType,
}

impl MediaTrack {
    pub fn new(id: u64, track: MediaStreamTrack, caps: MediaType) -> Rc<Self> {
        Rc::new(Self { id, track, caps })
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn track(&self) -> &MediaStreamTrack {
        &self.track
    }

    pub fn caps(&self) -> &MediaType {
        &self.caps
    }
}

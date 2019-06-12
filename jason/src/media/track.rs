use std::rc::Rc;

use medea_client_api_proto::MediaType;
use web_sys::MediaStreamTrack;

/// [`MediaStreamTrack`] wrapper.
pub struct MediaTrack {
    id: u64,
    track: MediaStreamTrack,
    caps: MediaType,
}

impl MediaTrack {
    pub fn new(
        id: u64,
        track: MediaStreamTrack,
        caps: MediaType,
    ) -> MediaTrack {
        Self { id, track, caps }
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

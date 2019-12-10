//! [MediaStreamTrack][1] representation.
//!
//! [1]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack

use std::rc::Rc;

use medea_client_api_proto::TrackId as Id;
use web_sys::MediaStreamTrack;

use crate::media::TrackConstraints;

/// Representation of [MediaStreamTrack][1].
///
/// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack
pub struct MediaTrack {
    id: Id,
    track: MediaStreamTrack,
    caps: TrackConstraints,
}

impl MediaTrack {
    /// Instantiates new [`MediaTrack`].
    pub fn new(
        id: Id,
        track: MediaStreamTrack,
        caps: TrackConstraints,
    ) -> Rc<Self> {
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
    pub fn caps(&self) -> &TrackConstraints {
        &self.caps
    }

    /// Checks if underlying [`MediaStreamTrack`] is enabled.
    pub fn is_enabled(&self) -> bool {
        self.track.enabled()
    }

    /// Enables or disables underlying [`MediaStreamTrack`].
    pub fn set_enabled(&self, enabled: bool) {
        self.track.set_enabled(enabled)
    }
}

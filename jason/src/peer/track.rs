//! [MediaStreamTrack][1] representation.
//!
//! [1]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack

use std::rc::Rc;

// use derive_more::From;
use medea_client_api_proto::TrackId as Id;

use crate::media::{MediaStreamTrack, TrackConstraints};

/// Representation of [MediaStreamTrack][1].
///
/// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack
pub struct PeerMediaTrack {
    id: Id,
    track: MediaStreamTrack,
    caps: TrackConstraints,
}

impl PeerMediaTrack {
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

    /// Returns [`TrackConstraints`] of this [`MediaTrack`].
    pub fn caps(&self) -> &TrackConstraints {
        &self.caps
    }

    // /// Checks if underlying [`MediaStreamTrack`] is enabled.
    // pub fn is_enabled(&self) -> bool {
    //     self.track.as_ref().enabled()
    // }
    //
    /// Enables or disables underlying [`MediaStreamTrack`].
    pub fn set_enabled(&self, enabled: bool) {
        self.track.as_ref().set_enabled(enabled)
    }

    // /// Sets [`MediaStreamTrack`] enabled property basing on the provided
    // /// `mute_state`.
    // #[inline]
    // pub fn set_enabled_by_mute_state(&self, mute_state: StableMuteState) {
    //     match mute_state {
    //         StableMuteState::Muted => {
    //             self.set_enabled(false);
    //         }
    //         StableMuteState::NotMuted => {
    //             self.set_enabled(true);
    //         }
    //     }
    // }
}

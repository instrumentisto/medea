//! [MediaStreamTrack][1] representation.
//!
//! [1]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack

use std::{cell::RefCell, ops::Not, rc::Rc};

use futures::StreamExt;
use medea_client_api_proto::{self as proto, TrackId as Id};
use medea_reactive::{Dropped, Reactive};
use tracerr::Traced;
use wasm_bindgen_futures::spawn_local;
use web_sys::MediaStreamTrack;

use crate::media::TrackConstraints;

/// Mute state of [`MediaTrack`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MutedState {
    /// [`MediaTrack`] is unmuted.
    Unmuted,

    /// [`MediaTrack`] should be unmuted, but awaits server permission.
    Unmuting,

    /// [`MediaTrack`] should be muted, but awaits server permission.
    Muting,

    /// [`MediaTrack`] is muted.
    Muted,
}

impl MutedState {
    /// Returns [`MutedState`] which should be set while transition to this
    /// [`MutedState`].
    pub fn proccessing_state(self) -> Self {
        match self {
            Self::Unmuted => Self::Unmuting,
            Self::Muted => Self::Muting,
            _ => self,
        }
    }
}

impl From<bool> for MutedState {
    fn from(is_muted: bool) -> Self {
        if is_muted {
            Self::Muted
        } else {
            Self::Unmuted
        }
    }
}

impl Not for MutedState {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            Self::Muted => Self::Unmuted,
            Self::Unmuted => Self::Muted,
            Self::Unmuting => Self::Muting,
            Self::Muting => Self::Unmuting,
        }
    }
}

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

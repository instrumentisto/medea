//! Snapshot for the `Track` object.

use serde::{Deserialize, Serialize};

use crate::{Direction, MediaType, TrackId, TrackPatch};

/// Snapshot of the state for the `MediaTrack`.
#[derive(Clone, Eq, PartialEq, Debug, Deserialize, Serialize)]
pub struct TrackSnapshot {
    /// ID of this [`TrackSnapshot`].
    pub id: TrackId,

    /// If `true` then `MediaTrack` is muted.
    pub is_muted: bool,

    /// Direction of `MediaTrack`.
    pub direction: Direction,

    /// Media type of `MediaTrack`.
    pub media_type: MediaType,
}

/// Accessor to the `Track` snapshot objects.
///
/// For this trait is implemented `CommandHandler` and
/// `EventHandler` which will be used on the Web Client side and on the Media
/// Server side. But real snapshot objects are different on the Web Client and
/// on the Media Server, so this abstraction is needed.
pub trait TrackSnapshotAccessor {
    /// Returns new `MediaTrack` with provided data.
    fn new(
        id: TrackId,
        is_muted: bool,
        direction: Direction,
        media_type: MediaType,
    ) -> Self;

    /// Patches this `MediaTrack` by [`TrackPatch`].
    fn patch(&mut self, patch: TrackPatch) {
        if let Some(is_muted) = patch.is_muted {
            self.set_is_muted(is_muted);
        }
    }

    /// Sets `MediaTrack` mute state.
    ///
    /// If `true` then `MediaTrack` is muted.
    fn set_is_muted(&mut self, is_muted: bool);

    /// Updates `MediaTrack` by the provided [`TrackSnapshot`].
    fn update_snapshot(&mut self, snapshot: TrackSnapshot) {
        self.set_is_muted(snapshot.is_muted);
    }
}

impl TrackSnapshotAccessor for TrackSnapshot {
    fn new(
        id: TrackId,
        is_muted: bool,
        direction: Direction,
        media_type: MediaType,
    ) -> Self {
        Self {
            id,
            is_muted,
            direction,
            media_type,
        }
    }

    fn set_is_muted(&mut self, is_muted: bool) {
        self.is_muted = is_muted;
    }
}

use serde::{Deserialize, Serialize};

use crate::{Direction, MediaType, TrackId, TrackPatch};

#[derive(Clone, Eq, PartialEq, Debug, Deserialize, Serialize)]
pub struct TrackSnapshot {
    pub id: TrackId,
    pub is_muted: bool,
    pub direction: Direction,
    pub media_type: MediaType,
}

pub trait TrackSnapshotAccessor {
    fn new(
        id: TrackId,
        is_muted: bool,
        direction: Direction,
        media_type: MediaType,
    ) -> Self;

    fn update(&mut self, patch: TrackPatch) {
        if let Some(is_muted) = patch.is_muted {
            self.set_is_muted(is_muted);
        }
    }

    fn set_is_muted(&mut self, is_muted: bool);

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

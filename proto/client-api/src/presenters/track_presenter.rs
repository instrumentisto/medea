use std::{cell::RefCell, collections::HashMap, rc::Rc};

use futures::{Stream, StreamExt};
use medea_reactive::{
    collections::{vec::ObservableVec, ObservableHashMap},
    Observable, ObservableCell,
};
use serde::{Serialize, Deserialize};

use crate::{
    Direction, EventHandler, IceCandidate, IceServer, MediaType, PeerId, Track,
    TrackId, TrackPatch,
};

pub struct TrackSnapshot {
    pub id: TrackId,
    pub is_muted: bool,
    pub direction: Direction,
    pub media_type: MediaType,
}

pub trait TrackSnapshotAccessor {
    fn new(id: TrackId, is_muted: bool, direction: Direction, media_type: MediaType) -> Self;

    fn update(&mut self, patch: TrackPatch) {
        if let Some(is_muted) = patch.is_muted {
            self.set_is_muted(is_muted);
        }
    }

    fn set_is_muted(&mut self, is_muted: bool);

    fn get_direction(&self) -> &Direction;

    fn get_media_type(&self) -> &MediaType;

    fn get_is_muted(&self) -> bool;

    fn get_id(&self) -> TrackId;
}

impl TrackSnapshotAccessor for TrackSnapshot {
    fn new(id: TrackId, is_muted: bool, direction: Direction, media_type: MediaType) -> Self {
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

    fn get_direction(&self) -> &Direction {
        &self.direction
    }

    fn get_media_type(&self) -> &MediaType {
        &self.media_type
    }

    fn get_is_muted(&self) -> bool {
        self.is_muted
    }

    fn get_id(&self) -> TrackId {
        self.id
    }
}





#[derive(Debug)]
pub struct TrackPresenter {
    pub(super) id: TrackId,
    pub(super) is_muted: ObservableCell<bool>,
    pub(super) direction: Direction,
    pub(super) media_type: MediaType,
}

impl TrackPresenter {
    pub fn update(&mut self, patch: TrackPatch) {
        if let Some(is_muted) = patch.is_muted {
            self.is_muted.set(is_muted);
        }
    }

    pub fn on_track_update(&self) -> impl Stream<Item = bool> {
        self.is_muted.subscribe()
    }

    pub fn get_direction(&self) -> &Direction {
        &self.direction
    }

    pub fn get_media_type(&self) -> &MediaType {
        &self.media_type
    }

    pub fn get_is_muted(&self) -> bool {
        self.is_muted.get()
    }

    pub fn get_id(&self) -> TrackId {
        self.id
    }
}

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use futures::{Stream, StreamExt};
use medea_reactive::{
    collections::{vec::ObservableVec, ObservableHashMap},
    Observable, ObservableCell,
};
use serde::{Deserialize, Serialize};

use crate::{
    Direction, EventHandler, IceCandidate, IceServer, MediaType, PeerId, Track,
    TrackId, TrackPatch,
};

use crate::snapshots::track::{TrackSnapshot, TrackSnapshotAccessor};

#[derive(Debug)]
pub struct PeerSnapshot {
    pub id: PeerId,
    pub sdp_offer: Option<String>,
    pub sdp_answer: Option<String>,
    pub tracks: HashMap<TrackId, TrackSnapshot>,
    pub ice_servers: Vec<IceServer>,
    pub is_force_relayed: bool,
    pub ice_candidates: Vec<IceCandidate>,
}

pub trait PeerSnapshotAccessor {
    type Track: TrackSnapshotAccessor;

    fn new(
        id: PeerId,
        sdp_offer: Option<String>,
        ice_servers: Vec<IceServer>,
        is_force_relayed: bool,
        tracks: HashMap<TrackId, Self::Track>,
    ) -> Self;

    fn set_sdp_answer(&mut self, sdp_answer: String);

    fn add_ice_candidate(&mut self, ice_candidate: IceCandidate);

    fn update_track<F>(&mut self, track_id: TrackId, update_fn: F)
    where
        F: FnOnce(Option<&mut Self::Track>);

    fn update_tracks(&mut self, patches: Vec<TrackPatch>) {
        for patch in patches {
            self.update_track(patch.id, |track| {
                if let Some(track) = track {
                    track.update(patch);
                }
            });
        }
    }
}

impl PeerSnapshotAccessor for PeerSnapshot {
    type Track = TrackSnapshot;

    fn new(
        id: PeerId,
        sdp_offer: Option<String>,
        ice_servers: Vec<IceServer>,
        is_force_relayed: bool,
        tracks: HashMap<TrackId, Self::Track>,
    ) -> Self {
        Self {
            id,
            sdp_offer,
            ice_servers,
            is_force_relayed,
            tracks,
            sdp_answer: None,
            ice_candidates: vec![],
        }
    }

    fn set_sdp_answer(&mut self, sdp_answer: String) {
        self.sdp_answer = Some(sdp_answer);
    }

    fn add_ice_candidate(&mut self, ice_candidate: IceCandidate) {
        self.ice_candidates.push(ice_candidate);
    }

    fn update_track<F>(&mut self, track_id: TrackId, update_fn: F)
    where
        F: FnOnce(Option<&mut Self::Track>),
    {
        (update_fn)(self.tracks.get_mut(&track_id));
    }
}

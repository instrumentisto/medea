use std::collections::{hash_map::RandomState, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{IceCandidate, IceServer, PeerId, TrackId, TrackPatch};

use super::{TrackSnapshot, TrackSnapshotAccessor};

#[derive(Clone, Eq, PartialEq, Debug, Deserialize, Serialize)]
pub struct PeerSnapshot {
    pub id: PeerId,
    pub sdp_offer: Option<String>,
    pub sdp_answer: Option<String>,
    pub tracks: HashMap<TrackId, TrackSnapshot>,
    pub ice_servers: HashSet<IceServer>,
    pub is_force_relayed: bool,
    pub ice_candidates: HashSet<IceCandidate>,
}

pub trait PeerSnapshotAccessor {
    type Track: TrackSnapshotAccessor;

    fn new(
        id: PeerId,
        sdp_offer: Option<String>,
        ice_servers: HashSet<IceServer>,
        is_force_relayed: bool,
        tracks: HashMap<TrackId, Self::Track>,
    ) -> Self;

    fn set_sdp_answer(&mut self, sdp_answer: Option<String>);

    fn set_sdp_offer(&mut self, sdp_offer: Option<String>);

    fn set_ice_servers(&mut self, ice_servers: HashSet<IceServer>);

    fn set_is_force_related(&mut self, is_force_relayed: bool);

    fn set_ice_candidates(&mut self, ice_candidates: HashSet<IceCandidate>);

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

    fn update_snapshot(&mut self, snapshot: PeerSnapshot) {
        self.set_sdp_answer(snapshot.sdp_answer);
        self.set_sdp_offer(snapshot.sdp_offer);
        self.set_ice_candidates(snapshot.ice_candidates);
        self.set_ice_servers(snapshot.ice_servers);
        self.set_is_force_related(snapshot.is_force_relayed);

        for (track_id, track_snapshot) in snapshot.tracks {
            self.update_track(track_id, |track| {
                if let Some(track) = track {
                    track.update_snapshot(track_snapshot);
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
        ice_servers: HashSet<IceServer>,
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
            ice_candidates: HashSet::new(),
        }
    }

    fn set_sdp_answer(&mut self, sdp_answer: Option<String>) {
        self.sdp_answer = sdp_answer;
    }

    fn set_sdp_offer(&mut self, sdp_offer: Option<String>) {
        self.sdp_offer = sdp_offer
    }

    fn set_ice_servers(
        &mut self,
        ice_servers: HashSet<IceServer, RandomState>,
    ) {
        self.ice_servers = ice_servers;
    }

    fn set_is_force_related(&mut self, is_force_relayed: bool) {
        self.is_force_relayed = is_force_relayed;
    }

    fn set_ice_candidates(
        &mut self,
        ice_candidates: HashSet<IceCandidate, RandomState>,
    ) {
        self.ice_candidates = ice_candidates;
    }

    fn add_ice_candidate(&mut self, ice_candidate: IceCandidate) {
        self.ice_candidates.insert(ice_candidate);
    }

    fn update_track<F>(&mut self, track_id: TrackId, update_fn: F)
    where
        F: FnOnce(Option<&mut Self::Track>),
    {
        (update_fn)(self.tracks.get_mut(&track_id));
    }
}

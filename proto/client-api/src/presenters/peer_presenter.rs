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

use super::TrackPresenter;
use crate::presenters::track_presenter::{TrackSnapshot, TrackSnapshotAccessor};

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

    fn new(id: PeerId, sdp_offer: Option<String>, ice_servers: Vec<IceServer>, is_force_relayed: bool, tracks: HashMap<TrackId, Self::Track>) -> Self;

    fn set_sdp_answer(&mut self, sdp_answer: String);

    fn add_ice_candidate(&mut self, ice_candidate: IceCandidate);

    fn update_track<F>(&mut self, track_id: TrackId, update_fn: F) where F: FnOnce(Option<&mut Self::Track>);

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

    fn new(id: PeerId, sdp_offer: Option<String>, ice_servers: Vec<IceServer>, is_force_relayed: bool, tracks: HashMap<TrackId, Self::Track>) -> Self {
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

    fn update_track<F>(&mut self, track_id: TrackId, update_fn: F) where F: FnOnce(Option<&mut Self::Track>) {
        (update_fn)(self.tracks.get_mut(&track_id));
    }
}

#[derive(Debug)]
pub struct PeerPresenter {
    pub(super) id: PeerId,
    pub(super) sdp_offer: Observable<Option<String>>,
    pub(super) sdp_answer: Observable<Option<String>>,
    pub(super) tracks: HashMap<TrackId, Rc<RefCell<TrackPresenter>>>,
    pub(super) ice_servers: ObservableVec<IceServer>,
    pub(super) is_force_relayed: Observable<bool>,
    pub(super) ice_candidates: ObservableVec<IceCandidate>,
}

impl PeerPresenter {
    pub fn on_sdp_answer_made(&self) -> impl Stream<Item = String> {
        self.sdp_answer.subscribe().filter_map(|new_sdp_answer| {
            Box::pin(async move { new_sdp_answer })
        })
    }

    pub fn on_ice_candidate_discovered(
        &self,
    ) -> impl Stream<Item = IceCandidate> {
        self.ice_candidates.on_push()
    }

    pub fn set_sdp_answer(&mut self, sdp_answer: String) {
        *self.sdp_answer.borrow_mut() = Some(sdp_answer);
    }

    pub fn add_ice_candidate(&mut self, ice_candidate: IceCandidate) {
        self.ice_candidates.push(ice_candidate);
    }

    pub fn update_tracks(&mut self, patches: Vec<TrackPatch>) {
        for patch in patches {
            if let Some(track) = self.tracks.get(&patch.id) {
                track.borrow_mut().update(patch);
            }
        }
    }

    pub fn get_ice_servers(&self) -> &[IceServer] {
        self.ice_servers.as_ref()
    }

    pub fn get_is_force_relayed(&self) -> bool {
        *self.is_force_relayed
    }

    pub fn get_sdp_offer(&self) -> &Option<String> {
        &self.sdp_offer
    }

    pub fn get_tracks(&self) -> Vec<Rc<RefCell<TrackPresenter>>> {
        self.tracks.values().cloned().collect()
    }

    pub fn get_id(&self) -> PeerId {
        self.id
    }
}

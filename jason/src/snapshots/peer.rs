use futures::{Stream, StreamExt as _};
use medea_client_api_proto::{
    snapshots::peer::PeerSnapshotAccessor, IceCandidate, IceServer, PeerId,
    TrackId, TrackPatch,
};
use medea_reactive::{collections::vec::ObservableVec, Observable};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

use super::ObservableTrackSnapshot;
use medea_client_api_proto::snapshots::{
    peer::PeerSnapshot, track::TrackSnapshotAccessor,
};
use medea_reactive::collections::ObservableHashSet;
use std::collections::HashSet;
use wasm_bindgen::__rt::std::collections::hash_map::RandomState;

#[derive(Debug)]
pub struct ObservablePeerSnapshot {
    pub(super) id: PeerId,
    pub(super) sdp_offer: Observable<Option<String>>,
    pub(super) sdp_answer: Observable<Option<String>>,
    pub(super) tracks: HashMap<TrackId, Rc<RefCell<ObservableTrackSnapshot>>>,
    pub(super) ice_servers: ObservableHashSet<IceServer>,
    pub(super) is_force_relayed: Observable<bool>,
    pub(super) ice_candidates: ObservableHashSet<IceCandidate>,
}

impl ObservablePeerSnapshot {
    pub fn on_sdp_answer_made(&self) -> impl Stream<Item = String> {
        self.sdp_answer.subscribe().filter_map(|new_sdp_answer| {
            Box::pin(async move { new_sdp_answer })
        })
    }

    pub fn on_ice_candidate_discovered(
        &self,
    ) -> impl Stream<Item = IceCandidate> {
        self.ice_candidates.on_insert()
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

    pub fn get_ice_servers(&self) -> Vec<IceServer> {
        self.ice_servers.iter().cloned().collect()
    }

    pub fn get_is_force_relayed(&self) -> bool {
        *self.is_force_relayed
    }

    pub fn get_sdp_offer(&self) -> &Option<String> {
        &self.sdp_offer
    }

    pub fn get_tracks(&self) -> Vec<Rc<RefCell<ObservableTrackSnapshot>>> {
        self.tracks.values().cloned().collect()
    }

    pub fn get_id(&self) -> PeerId {
        self.id
    }
}

impl PeerSnapshotAccessor for ObservablePeerSnapshot {
    type Track = ObservableTrackSnapshot;

    fn new(
        id: PeerId,
        sdp_offer: Option<String>,
        ice_servers: HashSet<IceServer>,
        is_force_relayed: bool,
        tracks: HashMap<TrackId, Self::Track>,
    ) -> Self {
        ObservablePeerSnapshot {
            id,
            sdp_answer: Observable::new(None),
            sdp_offer: Observable::new(sdp_offer),
            ice_servers: ice_servers.into(),
            is_force_relayed: Observable::new(is_force_relayed),
            ice_candidates: ObservableHashSet::new(),
            tracks: tracks
                .into_iter()
                .map(|(id, track)| (id, Rc::new(RefCell::new(track))))
                .collect(),
        }
    }

    fn set_sdp_answer(&mut self, sdp_answer: Option<String>) {
        *self.sdp_answer.borrow_mut() = sdp_answer;
    }

    fn set_sdp_offer(&mut self, sdp_offer: Option<String>) {
        *self.sdp_offer.borrow_mut() = sdp_offer;
    }

    fn set_ice_servers(&mut self, ice_servers: HashSet<IceServer>) {
        self.ice_servers.update(ice_servers);
    }

    fn set_is_force_related(&mut self, is_force_relayed: bool) {
        *self.is_force_relayed.borrow_mut() = is_force_relayed;
    }

    fn set_ice_candidates(
        &mut self,
        ice_candidates: HashSet<IceCandidate, RandomState>,
    ) {
        self.ice_candidates.update(ice_candidates);
    }

    fn add_ice_candidate(&mut self, ice_candidate: IceCandidate) {
        self.ice_candidates.push(ice_candidate);
    }

    fn update_track<F>(&mut self, track_id: TrackId, update_fn: F)
    where
        F: FnOnce(Option<&mut Self::Track>),
    {
        if let Some(track) = self.tracks.get(&track_id) {
            (update_fn)(Some(&mut track.borrow_mut()));
        } else {
            (update_fn)(None);
        }
    }
}

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

use futures::{Stream, StreamExt as _};
use medea_client_api_proto::{
    snapshots::peer::PeerSnapshotAccessor, IceCandidate, IceServer, PeerId,
    TrackId,
};
use medea_reactive::{collections::ObservableHashSet, Observable};

use super::ObservableTrackSnapshot;

#[derive(Debug)]
pub struct ObservablePeerSnapshot {
    id: PeerId,
    sdp_offer: Observable<Option<String>>,
    sdp_answer: Observable<Option<String>>,
    tracks: HashMap<TrackId, Rc<RefCell<ObservableTrackSnapshot>>>,
    ice_servers: ObservableHashSet<IceServer>,
    is_force_relayed: Observable<bool>,
    ice_candidates: ObservableHashSet<IceCandidate>,
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

    fn set_ice_candidates(&mut self, ice_candidates: HashSet<IceCandidate>) {
        self.ice_candidates.update(ice_candidates);
    }

    fn add_ice_candidate(&mut self, ice_candidate: IceCandidate) {
        self.ice_candidates.insert(ice_candidate);
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

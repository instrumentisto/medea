use std::{cell::RefCell, collections::HashMap, rc::Rc};

use futures::{Stream, StreamExt};
use medea_client_api_proto::{
    Direction, EventHandler, IceCandidate, IceServer, MediaType, PeerId, Track,
    TrackId, TrackPatch,
};
use medea_reactive::{
    collections::{vec::ObservableVec, ObservableHashMap},
    Observable, ObservableCell,
};

pub struct RoomPresenter {
    peers: ObservableHashMap<PeerId, Rc<RefCell<PeerPresenter>>>,
}

impl RoomPresenter {
    pub fn new() -> Self {
        Self {
            peers: ObservableHashMap::new(),
        }
    }
}

/// RPC events handling.
impl EventHandler for RoomPresenter {
    type Output = ();

    fn on_peer_created(
        &mut self,
        peer_id: PeerId,
        sdp_offer: Option<String>,
        tracks: Vec<Track>,
        ice_servers: Vec<IceServer>,
        is_force_relayed: bool,
    ) {
        let tracks = tracks
            .into_iter()
            .map(|track| {
                (
                    track.id,
                    Rc::new(RefCell::new(TrackPresenter {
                        id: track.id,
                        is_muted: ObservableCell::new(track.is_muted),
                        direction: track.direction,
                        media_type: track.media_type,
                    })),
                )
            })
            .collect();
        let peer_state = PeerPresenter {
            id: peer_id,
            sdp_answer: Observable::new(None),
            sdp_offer: Observable::new(sdp_offer),
            ice_servers: ice_servers.into(),
            is_force_relayed: Observable::new(is_force_relayed),
            ice_candidates: ObservableVec::new(),
            tracks,
        };

        self.peers
            .insert(peer_id, Rc::new(RefCell::new(peer_state)));
    }

    fn on_sdp_answer_made(&mut self, peer_id: PeerId, sdp_answer: String) {
        if let Some(peer) = self.peers.get(&peer_id) {
            peer.borrow_mut().set_sdp_answer(sdp_answer);
        }
    }

    fn on_ice_candidate_discovered(
        &mut self,
        peer_id: PeerId,
        candidate: IceCandidate,
    ) {
        if let Some(peer) = self.peers.get(&peer_id) {
            peer.borrow_mut().add_ice_candidate(candidate);
        }
    }

    fn on_peers_removed(&mut self, peer_ids: Vec<PeerId>) {
        for peer_id in peer_ids {
            self.peers.remove(&peer_id);
        }
    }

    fn on_tracks_updated(&mut self, peer_id: PeerId, tracks: Vec<TrackPatch>) {
        if let Some(peer) = self.peers.get(&peer_id) {
            peer.borrow_mut().update_tracks(tracks);
        }
    }
}

impl RoomPresenter {
    pub fn on_peer_created(
        &self,
    ) -> impl Stream<Item = (PeerId, Rc<RefCell<PeerPresenter>>)> {
        self.peers.on_insert()
    }

    pub fn on_peer_removed(
        &self,
    ) -> impl Stream<Item = (PeerId, Rc<RefCell<PeerPresenter>>)> {
        self.peers.on_remove()
    }
}

pub struct PeerPresenter {
    id: PeerId,
    sdp_offer: Observable<Option<String>>,
    sdp_answer: Observable<Option<String>>,
    tracks: HashMap<TrackId, Rc<RefCell<TrackPresenter>>>,
    ice_servers: ObservableVec<IceServer>,
    is_force_relayed: Observable<bool>,
    ice_candidates: ObservableVec<IceCandidate>,
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

#[derive(Debug)]
pub struct TrackPresenter {
    id: TrackId,
    is_muted: ObservableCell<bool>,
    direction: Direction,
    media_type: MediaType,
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

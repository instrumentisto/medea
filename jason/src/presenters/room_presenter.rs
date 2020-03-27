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

use crate::presenters::{PeerPresenter, TrackPresenter};

pub struct RoomPresenter {
    pub(super) peers: ObservableHashMap<PeerId, Rc<RefCell<PeerPresenter>>>,
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

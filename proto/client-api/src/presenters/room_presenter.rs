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

use super::{PeerPresenter, TrackPresenter};
use crate::presenters::peer_presenter::{PeerSnapshot, PeerSnapshotAccessor};

pub struct RoomSnapshot {
    pub peers: HashMap<PeerId, PeerSnapshot>,
}

pub trait RoomSnapshotAccessor {
    type Peer: PeerSnapshotAccessor;

    fn insert_peer(&mut self, peer_id: PeerId, peer: Self::Peer);

    fn remove_peer(&mut self, peer_id: PeerId);

    fn update_peer<F>(&mut self, peer_id: PeerId, update_fn: F) where F: FnOnce(Option<&mut Self::Peer>);
}

impl RoomSnapshotAccessor for RoomSnapshot {
    type Peer = PeerSnapshot;

    fn insert_peer(&mut self, peer_id: PeerId, peer: Self::Peer) {
        self.peers.insert(peer_id, peer);
    }

    fn remove_peer(&mut self, peer_id: PeerId) {
        self.peers.remove(&peer_id);
    }

    fn update_peer<F>(&mut self, peer_id: PeerId, update_fn: F) where F: FnOnce(Option<&mut Self::Peer>) {
        (update_fn)(self.peers.get_mut(&peer_id));
    }
}
use super::track_presenter::TrackSnapshotAccessor;

impl<R> EventHandler for R where R: RoomSnapshotAccessor {
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
                    <<R as RoomSnapshotAccessor>::Peer as PeerSnapshotAccessor>::Track::new(track.id, track.is_muted, track.direction, track.media_type),
                )
            })
            .collect();
        let peer = R::Peer::new(peer_id, sdp_offer, ice_servers, is_force_relayed, tracks);
        self.insert_peer(peer_id, peer);
    }

    fn on_sdp_answer_made(&mut self, peer_id: PeerId, sdp_answer: String) {
        self.update_peer(peer_id, move |peer| {
            if let Some(peer) = peer {
                peer.set_sdp_answer(sdp_answer);
            }
        });
    }

    fn on_ice_candidate_discovered(
        &mut self,
        peer_id: PeerId,
        candidate: IceCandidate,
    ) {
        self.update_peer(peer_id, move |peer| {
            if let Some(peer) = peer {
                peer.add_ice_candidate(candidate);
            }
        });
    }

    fn on_peers_removed(&mut self, peer_ids: Vec<PeerId>) {
        for peer_id in peer_ids {
            self.remove_peer(peer_id);
        }
    }

    fn on_tracks_updated(&mut self, peer_id: PeerId, tracks: Vec<TrackPatch>) {
        self.update_peer(peer_id, move |peer| {
            if let Some(peer) = peer {
                peer.update_tracks(tracks);
            }
        });
    }
}

#[derive(Debug)]
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

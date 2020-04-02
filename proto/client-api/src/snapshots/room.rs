use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{EventHandler, IceCandidate, IceServer, PeerId, Track, TrackPatch};

use crate::snapshots::peer::{PeerSnapshot, PeerSnapshotAccessor};

#[derive(Clone, Eq, PartialEq, Debug, Deserialize, Serialize)]
pub struct RoomSnapshot {
    pub peers: HashMap<PeerId, PeerSnapshot>,
}

impl RoomSnapshot {
    pub fn new() -> Self {
        Self {
            peers: HashMap::new(),
        }
    }
}

pub trait RoomSnapshotAccessor {
    type Peer: PeerSnapshotAccessor;

    fn insert_peer(&mut self, peer_id: PeerId, peer: Self::Peer);

    fn remove_peer(&mut self, peer_id: PeerId);

    fn update_peer<F>(&mut self, peer_id: PeerId, update_fn: F)
    where
        F: FnOnce(Option<&mut Self::Peer>);

    // TODO: break into pieces
    fn update_snapshot(&mut self, snapshot: RoomSnapshot);
}

impl RoomSnapshotAccessor for RoomSnapshot {
    type Peer = PeerSnapshot;

    fn insert_peer(&mut self, peer_id: PeerId, peer: Self::Peer) {
        self.peers.insert(peer_id, peer);
    }

    fn remove_peer(&mut self, peer_id: PeerId) {
        self.peers.remove(&peer_id);
    }

    fn update_peer<F>(&mut self, peer_id: PeerId, update_fn: F)
    where
        F: FnOnce(Option<&mut Self::Peer>),
    {
        (update_fn)(self.peers.get_mut(&peer_id));
    }

    fn update_snapshot(&mut self, snapshot: RoomSnapshot) {
        for (peer_id, peer_snapshot) in snapshot.peers {
            if let Some(peer) = self.peers.get_mut(&peer_id) {
                peer.update_snapshot(peer_snapshot);
            }
        }
    }
}
use super::track::TrackSnapshotAccessor;

impl<R> EventHandler for R
where
    R: RoomSnapshotAccessor,
{
    type Output = ();

    fn on_peer_created(
        &mut self,
        peer_id: PeerId,
        sdp_offer: Option<String>,
        tracks: Vec<Track>,
        ice_servers: HashSet<IceServer>,
        is_force_relayed: bool,
    ) {
        type Peer<R> = <R as RoomSnapshotAccessor>::Peer;
        type Track<R> = <Peer<R> as PeerSnapshotAccessor>::Track;

        let tracks = tracks
            .into_iter()
            .map(|track| {
                (
                    track.id,
                    Track::<R>::new(
                        track.id,
                        track.is_muted,
                        track.direction,
                        track.media_type,
                    ),
                )
            })
            .collect();
        let peer = R::Peer::new(
            peer_id,
            sdp_offer,
            ice_servers,
            is_force_relayed,
            tracks,
        );
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

    fn on_restore_state(&mut self, snapshot: RoomSnapshot) {
        self.update_snapshot(snapshot);
    }
}

use medea_client_api_proto::{PeerConnectionState, PeerId, TrackId};
use std::{
    cell::RefCell,
    collections::HashMap,
    rc::{Rc, Weak},
};

use super::EventSender;
use crate::{
    media::PeerStateMachine,
    signalling::peers::{metrics::RtcStatsHandler, PeersMetricsEvent},
};
use futures::stream::LocalBoxStream;
use medea_client_api_proto::stats::RtcStat;
use std::collections::hash_map::RandomState;

#[derive(Debug)]
pub struct TracksDesyncDetector {
    peers: HashMap<PeerId, Rc<RefCell<Peer>>>,
    event_tx: EventSender,
}

#[derive(Debug)]
struct Peer {
    id: PeerId,
    partner: Weak<RefCell<Peer>>,
    transceivers_statuses: HashMap<TrackId, bool>,
}

impl Peer {
    pub fn new_pair(
        first_peer_id: PeerId,
        second_peer_id: PeerId,
    ) -> (Rc<RefCell<Self>>, Rc<RefCell<Self>>) {
        let first_peer = Rc::new(RefCell::new(Peer {
            id: first_peer_id,
            partner: Weak::default(),
            transceivers_statuses: HashMap::new(),
        }));
        let second_peer = Rc::new(RefCell::new(Peer {
            id: second_peer_id,
            partner: Rc::downgrade(&first_peer),
            transceivers_statuses: HashMap::new(),
        }));
        first_peer.borrow_mut().partner = Rc::downgrade(&second_peer);

        (first_peer, second_peer)
    }
}

impl TracksDesyncDetector {
    pub fn new() -> Self {
        Self {
            peers: HashMap::new(),
            event_tx: EventSender::new(),
        }
    }
}

impl RtcStatsHandler for TracksDesyncDetector {
    fn register_peer(&mut self, peer: &PeerStateMachine) {
        let peer_id = peer.id();
        if !self.peers.contains_key(&peer_id) {
            let partner_peer_id = peer.partner_peer_id();
            let (peer, partner_peer) = Peer::new_pair(peer_id, partner_peer_id);
            self.peers.insert(peer_id, peer);
            self.peers.insert(partner_peer_id, partner_peer);
        }
    }

    fn unregister_peers(&mut self, peers_ids: &[PeerId]) {
        for peer_id in peers_ids {
            self.peers.remove(peer_id);
        }
    }

    fn update_peer(&mut self, peer: &PeerStateMachine) {}

    fn check(&mut self) {}

    fn add_stats(&mut self, peer_id: PeerId, stats: &[RtcStat]) {}

    fn update_peer_connection_state(
        &mut self,
        peer_id: PeerId,
        state: PeerConnectionState,
    ) {
    }

    fn subscribe(&mut self) -> LocalBoxStream<'static, PeersMetricsEvent> {
        self.event_tx.subscribe()
    }

    fn update_transceivers_statuses(
        &mut self,
        peer_id: PeerId,
        transceivers_statuses: HashMap<TrackId, bool>,
    ) {
        if let Some(peer) = self.peers.get(&peer_id) {
            let mut peer_ref = peer.borrow_mut();
            let partner_peer = peer_ref.partner.upgrade().unwrap();
            let mut partner_peer_ref = partner_peer.borrow_mut();

            let mut maybe_desynced = true;
            for (track_id, transceiver_status) in transceivers_statuses {
                peer_ref
                    .transceivers_statuses
                    .insert(track_id, transceiver_status);
            }
            if maybe_desynced {
                for (track_id, transceiver_status) in
                    &peer_ref.transceivers_statuses
                {
                    if let Some(partner_transceiver_status) =
                        partner_peer_ref.transceivers_statuses.get(track_id)
                    {
                        if partner_transceiver_status != transceiver_status {
                            self.event_tx.send_event(
                                PeersMetricsEvent::PeerTracksDesynced {
                                    peer_id: peer_ref.id,
                                },
                            );
                            break;
                        }
                    }
                }
            }
        }
    }
}

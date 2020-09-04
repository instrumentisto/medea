use std::collections::HashMap;

use futures::stream::LocalBoxStream;
use medea_client_api_proto::{PeerConnectionState, PeerId};

use crate::media::PeerStateMachine;

use super::{PeersMetricsEvent, RtcStatsHandler};
use crate::signalling::peers::metrics::EventSender;
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

#[derive(Debug)]
struct Peer {
    id: PeerId,
    partner_peer: Weak<RefCell<Peer>>,
    connection_state: PeerConnectionState,
}

impl Peer {
    pub fn new_pair(
        first_peer_id: PeerId,
        second_peer_id: PeerId,
    ) -> (Rc<RefCell<Peer>>, Rc<RefCell<Peer>>) {
        let first_peer = Rc::new(RefCell::new(Peer {
            id: first_peer_id,
            partner_peer: Weak::default(),
            connection_state: PeerConnectionState::New,
        }));
        let second_peer = Rc::new(RefCell::new(Peer {
            id: second_peer_id,
            partner_peer: Rc::downgrade(&first_peer),
            connection_state: PeerConnectionState::New,
        }));
        first_peer.borrow_mut().partner_peer = Rc::downgrade(&second_peer);

        (first_peer, second_peer)
    }

    pub fn partner_peer(&self) -> Rc<RefCell<Self>> {
        self.partner_peer.upgrade().unwrap()
    }

    pub fn update_connection_state(&mut self, new_state: PeerConnectionState) {
        self.connection_state = new_state;
    }

    pub fn id(&self) -> PeerId {
        self.id
    }
}

#[derive(Debug)]
pub struct IceRestartDetector {
    peers: HashMap<PeerId, Rc<RefCell<Peer>>>,
    event_sender: EventSender,
}

impl RtcStatsHandler for IceRestartDetector {
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
            if let Some(peer) = self.peers.remove(peer_id) {
                self.peers
                    .remove(&peer.borrow().partner_peer().borrow().id());
            }
        }
    }

    fn update_connection_state(
        &mut self,
        peer_id: PeerId,
        new_state: PeerConnectionState,
    ) {
        if let Some(peer) = self.peers.get(&peer_id) {
            if let PeerConnectionState::Failed = new_state {
                let old_state = peer.borrow().connection_state;
                match old_state {
                    PeerConnectionState::Connected
                    | PeerConnectionState::Disconnected => {
                        let partner_state = peer
                            .borrow()
                            .partner_peer()
                            .borrow()
                            .connection_state;
                        if let PeerConnectionState::Failed = partner_state {
                            self.event_sender.send_event(
                                PeersMetricsEvent::IceRestartNeeded { peer_id },
                            );
                        }
                    }
                    _ => (),
                }
            }
            peer.borrow_mut().update_connection_state(new_state);
        }
    }

    fn subscribe(&mut self) -> LocalBoxStream<'static, PeersMetricsEvent> {
        self.event_sender.subscribe()
    }
}

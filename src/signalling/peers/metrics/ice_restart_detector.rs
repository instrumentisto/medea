use std::collections::HashMap;

use futures::stream::LocalBoxStream;
use medea_client_api_proto::{PeerConnectionState, PeerId};

use crate::media::PeerStateMachine;

use super::{PeersMetricsEvent, RtcStatsHandler};
use crate::signalling::peers::metrics::EventSender;

use self::peer_state::PeerState;

mod peer_state {
    use std::{
        cell::RefCell,
        rc::{Rc, Weak},
    };

    use medea_client_api_proto::{PeerConnectionState, PeerId};

    #[derive(Debug)]
    struct Inner {
        id: PeerId,
        partner_peer: Weak<RefCell<Inner>>,
        connection_state: PeerConnectionState,
    }

    #[derive(Debug)]
    pub struct PeerState(Rc<RefCell<Inner>>);

    impl PeerState {
        pub fn new_pair(
            first_peer_id: PeerId,
            second_peer_id: PeerId,
        ) -> (Self, Self) {
            let first_peer = Rc::new(RefCell::new(Inner {
                id: first_peer_id,
                partner_peer: Weak::default(),
                connection_state: PeerConnectionState::New,
            }));
            let second_peer = Rc::new(RefCell::new(Inner {
                id: second_peer_id,
                partner_peer: Rc::downgrade(&first_peer),
                connection_state: PeerConnectionState::New,
            }));
            first_peer.borrow_mut().partner_peer = Rc::downgrade(&second_peer);

            (Self(first_peer), Self(second_peer))
        }

        pub fn partner_peer(&self) -> Self {
            Self(self.0.borrow().partner_peer.upgrade().unwrap())
        }

        pub fn connection_state(&self) -> PeerConnectionState {
            self.0.borrow().connection_state
        }

        pub fn update_connection_state(&self, new_state: PeerConnectionState) {
            self.0.borrow_mut().connection_state = new_state;
        }

        pub fn id(&self) -> PeerId {
            self.0.borrow().id
        }
    }
}

#[derive(Debug)]
pub struct IceRestartDetector {
    peers: HashMap<PeerId, PeerState>,
    event_sender: EventSender,
}

impl RtcStatsHandler for IceRestartDetector {
    fn register_peer(&mut self, peer: &PeerStateMachine) {
        let peer_id = peer.id();
        if !self.peers.contains_key(&peer_id) {
            let partner_peer_id = peer.partner_peer_id();
            let (peer, partner_peer) =
                PeerState::new_pair(peer_id, partner_peer_id);

            self.peers.insert(peer_id, peer);
            self.peers.insert(partner_peer_id, partner_peer);
        }
    }

    fn unregister_peers(&mut self, peers_ids: &[PeerId]) {
        for peer_id in peers_ids {
            if let Some(peer) = self.peers.remove(peer_id) {
                self.peers.remove(&peer.partner_peer().id());
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
                let old_state = peer.connection_state();
                match old_state {
                    PeerConnectionState::Connected
                    | PeerConnectionState::Disconnected => {
                        let partner_state =
                            peer.partner_peer().connection_state();
                        if let PeerConnectionState::Failed = partner_state {
                            self.event_sender.send_event(
                                PeersMetricsEvent::IceRestartNeeded { peer_id },
                            );
                        }
                    }
                    _ => (),
                }
            }
            peer.update_connection_state(new_state);
        }
    }

    fn subscribe(&mut self) -> LocalBoxStream<'static, PeersMetricsEvent> {
        self.event_sender.subscribe()
    }
}

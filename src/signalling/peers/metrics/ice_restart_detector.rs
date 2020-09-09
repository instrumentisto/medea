//! Implementation of the ICE restart detector.

use std::collections::HashMap;

use futures::stream::LocalBoxStream;
use medea_client_api_proto::{stats::RtcStat, PeerConnectionState, PeerId};

use crate::{
    log::prelude::*, media::PeerStateMachine,
    signalling::peers::metrics::EventSender,
};

use super::{PeersMetricsEvent, RtcStatsHandler};

use self::peer_state::PeerState;

/// Implementation of the ICE connection state of `PeerConnection`.
mod peer_state {
    use std::{
        cell::RefCell,
        rc::{Rc, Weak},
    };

    use medea_client_api_proto::{PeerConnectionState, PeerId};

    /// Inner of the [`PeerState`].
    #[derive(Debug)]
    struct Inner {
        /// [`PeerId`] of `PeerConnection` to which this [`PeerState`] belongs
        /// to.
        id: PeerId,

        /// Weak reference to the partner [`PeerState`].
        partner_peer: Weak<RefCell<Inner>>,

        /// Current [`PeerConnectionState`] of this `PeerConnection`.
        connection_state: PeerConnectionState,
    }

    /// ICE connection state of `PeerConnection`.
    #[derive(Debug)]
    pub struct PeerState(Rc<RefCell<Inner>>);

    impl PeerState {
        /// Returns new [`PeerState`] pair for the provided [`PeerId`]s.
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

        /// Returns [`PeerId`] of this [`PeerState`].
        pub fn id(&self) -> PeerId {
            self.0.borrow().id
        }

        /// Returns partner [`PeerState`].
        pub fn partner_peer(&self) -> Self {
            Self(self.0.borrow().partner_peer.upgrade().unwrap())
        }

        /// Returns current [`PeerConnectionState`] from this [`PeerState`].
        pub fn connection_state(&self) -> PeerConnectionState {
            self.0.borrow().connection_state
        }

        /// Updates [`PeerConnectionState`] of this [`PeerState`].
        pub fn update_connection_state(&self, new_state: PeerConnectionState) {
            self.0.borrow_mut().connection_state = new_state;
        }
    }
}

/// [`RtcStatsHandler`] responsible for the detecting ICE connection fails and
/// sending [`PeerMetricEvent::IceRestartNeeded`].
#[derive(Debug)]
pub struct IceRestartDetector {
    /// All [`PeerState`]s registered in this [`IceRestartDetector`].
    peers: HashMap<PeerId, PeerState>,

    /// [`PeerMetricsEvent`]s sender.
    event_tx: EventSender,
}

impl IceRestartDetector {
    /// Returns new [`IceRestartDetector`].
    pub fn new() -> Self {
        IceRestartDetector {
            peers: HashMap::new(),
            event_tx: EventSender::new(),
        }
    }
}

impl RtcStatsHandler for IceRestartDetector {
    /// Creates [`PeerState`] pair for the provided [`PeerStateMachine`] and
    /// it's partner [`PeerStateMachine`].
    ///
    /// If [`PeerState`] pair for the provided [`PeerStateMachine`] already
    /// exist, then nothing will be done.
    #[allow(clippy::map_entry)]
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

    /// Removes [`PeerMetric`]s with the provided [`PeerId`]s.
    fn unregister_peers(&mut self, peers_ids: &[PeerId]) {
        for peer_id in peers_ids {
            if let Some(peer) = self.peers.remove(peer_id) {
                self.peers.remove(&peer.partner_peer().id());
            }
        }
    }

    /// Does nothing.
    #[inline]
    fn update_peer(&mut self, _: &PeerStateMachine) {}

    /// Does nothing.
    #[inline]
    fn check(&mut self) {}

    /// Does nothing.
    #[inline]
    fn add_stats(&mut self, _: PeerId, _: &[RtcStat]) {}

    /// Updates [`PeerConnectionState`] in the [`PeerState`] with a provided
    /// [`PeerId`].
    ///
    /// Sends [`PeerMetricsEvent::IceRestartNeeded`] if [`PeerConnectionState`]
    /// goes to [`PeerConnectionState::Failed`] from
    /// [`PeerConnectionState::Connected`] or
    /// [`PeerConnectionState::Disconnected`].
    fn update_peer_connection_state(
        &mut self,
        peer_id: PeerId,
        new_state: PeerConnectionState,
    ) {
        debug!(
            "Receiver Peer [id = {}] connection state update: {:?}",
            peer_id, new_state
        );
        if let Some(peer) = self.peers.get(&peer_id) {
            if let PeerConnectionState::Failed = new_state {
                let old_state = peer.connection_state();
                match old_state {
                    PeerConnectionState::Connected
                    | PeerConnectionState::Disconnected => {
                        let partner_state =
                            peer.partner_peer().connection_state();
                        if let PeerConnectionState::Failed = partner_state {
                            debug!(
                                "Sending ICE restart for the Peer [id = {}].",
                                peer_id
                            );
                            self.event_tx.send_event(
                                PeersMetricsEvent::IceRestartNeeded { peer_id },
                            );
                        }
                    }
                    _ => (),
                }
            }
            peer.update_connection_state(new_state);
        } else {
            warn!("Peer [id = {}] not found.", peer_id);
        }
    }

    fn subscribe(&mut self) -> LocalBoxStream<'static, PeersMetricsEvent> {
        self.event_tx.subscribe()
    }
}

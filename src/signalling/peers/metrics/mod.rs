mod flowing_detector;
mod quality_meter;

use crate::{
    api::control::{
        callback::{MediaDirection, MediaType},
        RoomId,
    },
    media::{Peer, PeerStateMachine},
    signalling::peers::PeerTrafficWatcher,
};
use chrono::{DateTime, Utc};
use futures::{channel::mpsc, stream::LocalBoxStream};
use medea_client_api_proto::{stats::RtcStat, MemberId, PeerId};
use medea_macro::dispatchable;
use std::{
    any::TypeId,
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Arc,
};

pub use quality_meter::EstimatedConnectionQuality;

#[derive(Debug, Clone)]
pub struct EventSender(
    Rc<RefCell<Option<mpsc::UnboundedSender<PeersMetricsEvent>>>>,
);

impl EventSender {
    pub fn new() -> Self {
        Self(Rc::default())
    }

    pub fn send_event(&self, event: PeersMetricsEvent) {
        if let Some(tx) = self.0.borrow().as_ref() {
            let _ = tx.unbounded_send(event);
        }
    }

    pub fn is_connected(&self) -> bool {
        self.0.borrow().is_some()
    }

    pub fn subscribe(&self) -> LocalBoxStream<'static, PeersMetricsEvent> {
        let (tx, rx) = mpsc::unbounded();

        self.0.borrow_mut().replace(tx);

        Box::pin(rx)
    }
}

/// Events which [`PeersMetricsService`] can send to its subscriber.
#[dispatchable]
#[derive(Debug, Clone)]
pub enum PeersMetricsEvent {
    /// Some `MediaTrack`s with provided [`TrackMediaType`] doesn't flows.
    NoTrafficFlow {
        peer_id: PeerId,
        was_flowing_at: DateTime<Utc>,
        media_type: MediaType,
        direction: MediaDirection,
    },

    /// Stopped `MediaTrack` with provided [`MediaType`] and [`MediaDirection`]
    /// was started after stopping.
    TrafficFlows {
        peer_id: PeerId,
        media_type: MediaType,
        direction: MediaDirection,
    },

    /// [`EstimatedConnectionQuality`] updated.
    QualityMeterUpdate {
        /// [`MemberId`] of the [`Peer`] which's [`EstimatedConnectionQuality`]
        /// was updated.
        member_id: MemberId,

        /// [`MemberId`] of the partner [`Peer`].
        partner_member_id: MemberId,

        /// Actual [`EstimatedConnectionQuality`].
        quality_score: EstimatedConnectionQuality,
    },
}

#[derive(Debug)]
pub struct PeerMetricsService {
    event_tx: EventSender,
    handlers: Vec<Box<dyn MetricHandler>>,
}

impl PeerMetricsService {
    pub fn new(
        room_id: RoomId,
        peers_traffic_watcher: Arc<dyn PeerTrafficWatcher>,
    ) -> Self {
        use self::flowing_detector::FlowingDetector;

        let event_tx = EventSender::new();
        let handlers: Vec<Box<dyn MetricHandler>> =
            vec![Box::new(FlowingDetector::new(
                room_id,
                event_tx.clone(),
                peers_traffic_watcher,
            ))];

        Self { event_tx, handlers }
    }

    pub fn subscribe(&mut self) -> LocalBoxStream<'static, PeersMetricsEvent> {
        self.event_tx.subscribe()
    }

    pub fn check(&mut self) {
        for handler in &mut self.handlers {
            handler.check();
        }
    }

    pub fn register_peer(&mut self, peer: &PeerStateMachine) {
        for handler in &mut self.handlers {
            handler.register_peer(peer);
        }
    }

    pub fn unregister_peers(&mut self, peers_ids: &Vec<PeerId>) {
        for handler in &mut self.handlers {
            handler.unregister_peers(peers_ids);
        }
    }

    pub fn update_peer(&mut self, peer: &PeerStateMachine) {
        for handler in &mut self.handlers {
            handler.update_peer(peer);
        }
    }

    pub fn add_stats(&mut self, peer_id: PeerId, stats: &Vec<RtcStat>) {
        for handler in &mut self.handlers {
            handler.add_stat(peer_id, stats);
        }
    }
}

trait MetricHandler: std::fmt::Debug {
    fn register_peer(&mut self, peer_id: &PeerStateMachine);

    // TODO: HashSet
    fn unregister_peers(&mut self, peers_ids: &Vec<PeerId>);

    fn update_peer(&mut self, peer: &PeerStateMachine);

    fn check(&mut self);

    fn add_stat(&mut self, peer_id: PeerId, stats: &Vec<RtcStat>);
}

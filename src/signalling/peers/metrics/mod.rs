//! Implementation of the service which will distribute [`RtcStat`]s between
//! [`RtcStatsHandler`]s.
//!
//! Stores all implementations of the [`RtcStatsHandler`]s.

mod flowing_detector;
mod quality_meter;

use std::{cell::RefCell, fmt::Debug, rc::Rc, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use futures::{channel::mpsc, stream::LocalBoxStream};
use medea_client_api_proto::{
    stats::RtcStat, ConnectionQualityScore, MemberId, PeerId,
};
use medea_macro::dispatchable;

use crate::{
    api::control::{
        callback::{MediaDirection, MediaType},
        RoomId,
    },
    media::PeerStateMachine,
    signalling::peers::{
        metrics::quality_meter::QualityMeterStatsHandler, PeerTrafficWatcher,
    },
};

use self::flowing_detector::TrafficFlowDetector;

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

    /// [`ConnectionQualityScore`] updated.
    QualityMeterUpdate {
        /// [`MemberId`] of the [`Peer`] which's [`ConnectionQualityScore`]
        /// was updated.
        member_id: MemberId,

        /// [`MemberId`] of the partner [`Peer`].
        partner_member_id: MemberId,

        /// Actual [`ConnectionQualityScore`].
        quality_score: ConnectionQualityScore,
    },
}

/// An interface for dealing with [`RtcStat`]s handlers.
pub trait RtcStatsHandler: Debug {
    /// [`PeerMetricsService`] notifies [`RtcStatsHandler`] about new
    /// `PeerConnection`s creation.
    fn register_peer(&mut self, peer_id: &PeerStateMachine);

    /// [`RtcStatsHandler`] should stop tracking provided [`Peer`]s.
    fn unregister_peers(&mut self, peers_ids: &[PeerId]);

    /// [`RtcStatsHandler`] can update [`PeerStateMachine`]s internal
    /// representation.
    ///
    /// Must be called each time [`PeerStateMachine`] tracks set changes (some
    /// track was added or removed).
    fn update_peer(&mut self, peer: &PeerStateMachine);

    /// [`RtcStatsHandler`] can process all collected stats, re-calculate
    /// metrics and send [`PeerMetricsEvent`] (if it's needed).
    ///
    /// Will be called periodically by [`PeerMetricsService`].
    fn check(&mut self);

    /// [`PeerMetricsService`] provides new [`RtcStat`]s for the
    /// [`RtcStatsHandler`].
    fn add_stats(&mut self, peer_id: PeerId, stats: &[RtcStat]);
}

/// Service which is responsible for processing [`Peer`]s [`RtcStat`] metrics.
#[derive(Debug)]
pub struct PeerMetricsService {
    /// Sender of the [`PeerMetricsEvent`]s.
    event_tx: EventSender,

    /// All [`RtcStatsHandler`]s registered in this [`PeerMetricsService`].
    handlers: Vec<Box<dyn RtcStatsHandler>>,
}

impl PeerMetricsService {
    /// Creates new [`PeerMetricsService`], registers all needed
    /// [`RtcStatsHandler`]s.
    pub fn new(
        room_id: RoomId,
        peers_traffic_watcher: Arc<dyn PeerTrafficWatcher>,
        stats_ttl: Duration,
    ) -> Self {
        let event_tx = EventSender::new();
        let handlers: Vec<Box<dyn RtcStatsHandler>> = vec![
            Box::new(TrafficFlowDetector::new(
                room_id,
                event_tx.clone(),
                peers_traffic_watcher,
                stats_ttl,
            )),
            Box::new(QualityMeterStatsHandler::new(event_tx.clone())),
        ];

        Self { event_tx, handlers }
    }

    /// Returns [`Stream`] of [`PeerMetricsEvent`]s.
    ///
    /// Creating new subscription will invalidate previous, so there may be only
    /// one subscription. Events are not saved or buffered at sending side, so
    /// you won't receive any events happened before subscription was made.
    pub fn subscribe(&mut self) -> LocalBoxStream<'static, PeersMetricsEvent> {
        self.event_tx.subscribe()
    }
}

impl RtcStatsHandler for PeerMetricsService {
    /// Calls [`RtcStatsHandler::register_peer`] on the all registered
    /// [`MetricsHandler`]s.
    fn register_peer(&mut self, peer: &PeerStateMachine) {
        for handler in &mut self.handlers {
            handler.register_peer(peer);
        }
    }

    /// Calls [`RtcStatsHandler::unregister_peer`] on the all registered
    /// [`MetricsHandler`]s.
    fn unregister_peers(&mut self, peers_ids: &[PeerId]) {
        for handler in &mut self.handlers {
            handler.unregister_peers(peers_ids);
        }
    }

    /// Calls [`RtcStatsHandler::update_peer`] on the all registered
    /// [`MetricsHandler`]s.
    fn update_peer(&mut self, peer: &PeerStateMachine) {
        for handler in &mut self.handlers {
            handler.update_peer(peer);
        }
    }

    /// Calls [`RtcStatsHandler::check`] on the all registered
    /// [`MetricsHandler`]s.
    fn check(&mut self) {
        for handler in &mut self.handlers {
            handler.check();
        }
    }

    /// Calls [`RtcStatsHandler::add_stats`] on the all registered
    /// [`MetricsHandler`]s.
    fn add_stats(&mut self, peer_id: PeerId, stats: &[RtcStat]) {
        for handler in &mut self.handlers {
            handler.add_stats(peer_id, stats);
        }
    }
}

/// Sender for the [`PeersMetricsEvent`]s.
#[derive(Debug, Clone)]
struct EventSender(
    Rc<RefCell<Option<mpsc::UnboundedSender<PeersMetricsEvent>>>>,
);

impl EventSender {
    /// Returns new [`EventSender`].
    fn new() -> Self {
        Self(Rc::default())
    }

    /// Tries to send provided [`PeersMetricsEvent`] to the subscriber.
    ///
    /// If no one subscribed - does nothing.
    fn send_event(&self, event: PeersMetricsEvent) {
        if let Some(tx) = self.0.borrow().as_ref() {
            let _ = tx.unbounded_send(event);
        }
    }

    /// Returns [`Stream`] of [`PeerMetricsEvent`]s.
    ///
    /// Creating new subscription will invalidate previous, so there may be only
    /// one subscription. Events are not saved or buffered at sending side, so
    /// you won't receive any events happened before subscription was made.
    fn subscribe(&self) -> LocalBoxStream<'static, PeersMetricsEvent> {
        let (tx, rx) = mpsc::unbounded();
        self.0.borrow_mut().replace(tx);
        Box::pin(rx)
    }
}

#[cfg(test)]
mockall::mock! {
    pub PeerMetricsService {
        fn new(
            room_id: RoomId,
            peers_traffic_watcher: Arc<dyn PeerTrafficWatcher>,
            stats_ttl: Duration,
        ) -> Self;
        fn subscribe(&mut self) -> LocalBoxStream<'static, PeersMetricsEvent>;
    }
    trait RtcStatsHandler {
        fn register_peer(&mut self, peer: &PeerStateMachine);
        fn unregister_peers(&mut self, peers_ids: &[PeerId]);
        fn update_peer(&mut self, peer: &PeerStateMachine);
        fn check(&mut self);
        fn add_stats(&mut self, peer_id: PeerId, stats: &[RtcStat]);
    }
}

#[cfg(test)]
impl_debug_by_struct_name!(MockPeerMetricsService);

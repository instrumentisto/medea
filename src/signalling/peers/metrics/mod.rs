//! Implementation of the service which will distribute [`RtcStat`]s between
//! [`MetricHandler`]s.
//!
//! Stores all implementations of the [`MetricHandler`]s.

mod flowing_detector;
mod quality_meter;

use std::{cell::RefCell, rc::Rc, sync::Arc};

use chrono::{DateTime, Utc};
use futures::{channel::mpsc, stream::LocalBoxStream};
use medea_client_api_proto::{stats::RtcStat, MemberId, PeerId};
use medea_macro::dispatchable;

use crate::{
    api::control::{
        callback::{MediaDirection, MediaType},
        RoomId,
    },
    media::PeerStateMachine,
    signalling::peers::{
        metrics::quality_meter::QualityMeterService, PeerTrafficWatcher,
    },
};

use self::flowing_detector::FlowingDetector;

pub use quality_meter::EstimatedConnectionQuality;

/// Sender for the [`PeersMetricsEvent`]s.
#[derive(Debug, Clone)]
pub struct EventSender(
    Rc<RefCell<Option<mpsc::UnboundedSender<PeersMetricsEvent>>>>,
);

impl EventSender {
    /// Returns new [`EventSender`].
    pub fn new() -> Self {
        Self(Rc::default())
    }

    /// Tries to send provided [`PeersMetricsEvent`] to the subscriber.
    ///
    /// If no one subscribed - does nothing.
    pub fn send_event(&self, event: PeersMetricsEvent) {
        if let Some(tx) = self.0.borrow().as_ref() {
            let _ = tx.unbounded_send(event);
        }
    }

    /// Returns `true` if someone subscribed to this [`EventSender`].
    pub fn is_connected(&self) -> bool {
        self.0.borrow().is_some()
    }

    /// Returns [`Stream`] of [`PeerMetricsEvent`]s.
    ///
    /// Creating new subscription will invalidate previous, so there may be only
    /// one subscription. Events are not saved or buffered at sending side, so
    /// you won't receive any events happened before subscription was made.
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

/// Service which is responsible for processing [`Peer`]s [`RtcStat`] metrics.
#[derive(Debug)]
pub struct PeerMetricsService {
    /// Sender of the [`PeerMetricsEvent`]s.
    event_tx: EventSender,

    /// All [`MetricHandler`]s registered in this [`PeerMetricsService`].
    handlers: Vec<Box<dyn MetricHandler>>,
}

impl PeerMetricsService {
    /// Creates new [`PeerMetricsService`], registers all needed
    /// [`MetricHandler`]s.
    pub fn new(
        room_id: RoomId,
        peers_traffic_watcher: Arc<dyn PeerTrafficWatcher>,
    ) -> Self {
        let event_tx = EventSender::new();
        let handlers: Vec<Box<dyn MetricHandler>> = vec![
            Box::new(FlowingDetector::new(
                room_id,
                event_tx.clone(),
                peers_traffic_watcher,
            )),
            Box::new(QualityMeterService::new(event_tx.clone())),
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

    /// Calls [`MetricHandler::check`] on the all registered
    /// [`MetricsHandler`]s.
    pub fn check(&mut self) {
        for handler in &mut self.handlers {
            handler.check();
        }
    }

    /// Calls [`MetricHandler::register_peer`] on the all registered
    /// [`MetricsHandler`]s.
    pub fn register_peer(&mut self, peer: &PeerStateMachine) {
        for handler in &mut self.handlers {
            handler.register_peer(peer);
        }
    }

    /// Calls [`MetricHandler::unregister_peer`] on the all registered
    /// [`MetricsHandler`]s.
    pub fn unregister_peers(&mut self, peers_ids: &Vec<PeerId>) {
        for handler in &mut self.handlers {
            handler.unregister_peers(peers_ids);
        }
    }

    /// Calls [`MetricHandler::update_peer`] on the all registered
    /// [`MetricsHandler`]s.
    pub fn update_peer(&mut self, peer: &PeerStateMachine) {
        for handler in &mut self.handlers {
            handler.update_peer(peer);
        }
    }

    /// Calls [`MetricHandler::add_stats`] on the all registered
    /// [`MetricsHandler`]s.
    pub fn add_stats(&mut self, peer_id: PeerId, stats: &Vec<RtcStat>) {
        for handler in &mut self.handlers {
            handler.add_stat(peer_id, stats);
        }
    }
}

/// An interface for dealing with [`RtcStat`]s handlers.
trait MetricHandler: std::fmt::Debug {
    /// [`PeerMetricsService`] notifies [`MetricHandler`] about new
    /// `PeerConnection`s creation.
    fn register_peer(&mut self, peer_id: &PeerStateMachine);

    /// [`MetricHandler`] should stop tracking provided [`Peer`]s.
    fn unregister_peers(&mut self, peers_ids: &Vec<PeerId>);

    /// [`MetricHandler`] can update [`PeerStateMachine`]s internal
    /// representation.
    ///
    /// Must be called each time [`PeerStateMachine`] tracks set changes (some
    /// track was added or removed).
    fn update_peer(&mut self, peer: &PeerStateMachine);

    /// [`MetricHandler`] can process all collected stats, re-calculate metrics
    /// and send [`PeerMetricsEvent`] (if it's needed).
    ///
    /// Will be called periodically by [`PeerMetricsService`].
    fn check(&mut self);

    /// [`PeerMetricsService`] provides new [`RtcStat`]s for the
    /// [`MetricHandler`].
    fn add_stat(&mut self, peer_id: PeerId, stats: &Vec<RtcStat>);
}

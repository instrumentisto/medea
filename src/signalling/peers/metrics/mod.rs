//! Service which is responsible for processing [`Peer`]s [`RtcStat`] metrics.
//!
//! 1. You should register [`Peer`] via [`RtcStatsHandler::register_peer`].
//! 2. Use [`RtcStatsHandler::subscribe`] to subscribe to stats processing
//!    results.
//! 3. Provide [`Peer`]'s metrics to [`RtcStatsHandler::add_stats`].
//! 4. Call [`RtcStatsHandler::check`] with reasonable interval
//!    (~1-2 sec), to check for stale metrics.
//!
//! Stores [`RtcStatsHandler`]s implementors.
//!
//! [`Peer`]: crate::media::peer::Peer

mod connection_failure_detector;
mod flowing_detector;
mod quality_meter;

use std::{cell::RefCell, fmt::Debug, rc::Rc, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use futures::{
    channel::mpsc,
    stream::{self, LocalBoxStream, StreamExt as _},
};
use medea_client_api_proto::{
    stats::RtcStat, ConnectionQualityScore, MemberId, PeerConnectionState,
    PeerId, RoomId,
};
use medea_macro::dispatchable;

use crate::{
    api::control::callback::{MediaDirection, MediaType},
    media::PeerStateMachine,
    signalling::peers::{
        metrics::{
            connection_failure_detector::ConnectionFailureDetector,
            flowing_detector::TrafficFlowDetector,
            quality_meter::QualityMeterStatsHandler,
        },
        PeerTrafficWatcher,
    },
};

/// WebRTC statistics analysis results.
#[dispatchable]
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum PeersMetricsEvent {
    /// Some `MediaTrack`s with provided [`MediaType`] doesn't flows.
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
        /// [`MemberId`] of the [`Peer`] which [`ConnectionQualityScore`]
        /// was updated.
        ///
        /// [`Peer`]: crate::media::peer::Peer
        member_id: MemberId,

        /// [`MemberId`] of the partner [`Peer`].
        ///
        /// [`Peer`]: crate::media::peer::Peer
        partner_member_id: MemberId,

        /// Actual [`ConnectionQualityScore`].
        quality_score: ConnectionQualityScore,
    },

    /// One or more of the ICE transports on the connection is in the `failed`
    /// state.
    PeerConnectionFailed {
        /// [`PeerId`] of `PeerConnection`.
        peer_id: PeerId,
    },
}

/// [`RtcStatsHandler`] performs [`RtcStat`]s analysis.
#[cfg_attr(test, mockall::automock)]
pub trait RtcStatsHandler: Debug {
    /// Acknowledge [`RtcStatsHandler`] that new `Peer` was created, so
    /// [`RtcStatsHandler`] should track its [`RtcStat`]s.
    fn register_peer(&mut self, peer_id: &PeerStateMachine);

    /// [`RtcStatsHandler`] should stop tracking provided [`Peer`]s.
    ///
    /// [`Peer`]: crate::media::peer::Peer
    fn unregister_peers(&mut self, peers_ids: &[PeerId]);

    /// [`RtcStatsHandler`] can update [`PeerStateMachine`]s internal
    /// representation.
    ///
    /// Must be called each time [`PeerStateMachine`] tracks set changes (some
    /// track was added or removed).
    fn update_peer(&mut self, peer: &PeerStateMachine);

    /// [`RtcStatsHandler`] can process all collected stats, re-calculate
    /// metrics and send [`PeersMetricsEvent`] (if it's needed).
    ///
    /// Will be called periodically by [`PeerMetricsService`].
    fn check(&mut self);

    /// [`PeerMetricsService`] provides new [`RtcStat`]s for the
    /// [`RtcStatsHandler`].
    fn add_stats(&mut self, peer_id: PeerId, stats: &[RtcStat]);

    /// [`PeerMetricsService`] provides [`PeerConnectionState`] update for the
    /// [`RtcStatsHandler`].
    fn update_peer_connection_state(
        &mut self,
        peer_id: PeerId,
        state: PeerConnectionState,
    );

    /// Returns [`Stream`] of [`PeersMetricsEvent`]s.
    ///
    /// Creating new subscription will invalidate previous, so there may be only
    /// one subscription. Events are not saved or buffered at sending side, so
    /// you won't receive any events happened before subscription was made.
    ///
    /// [`Stream`]: futures::stream::Stream
    fn subscribe(&mut self) -> LocalBoxStream<'static, PeersMetricsEvent>;
}

#[cfg(test)]
impl_debug_by_struct_name!(MockRtcStatsHandler);

/// Service which is responsible for processing [`Peer`]s [`RtcStat`] metrics.
///
/// [`Peer`]: crate::media::peer::Peer
#[derive(Debug)]
pub struct PeerMetricsService {
    /// Sender of the [`PeersMetricsEvent`]s.
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
                peers_traffic_watcher,
                stats_ttl,
            )),
            Box::new(QualityMeterStatsHandler::new()),
            Box::new(ConnectionFailureDetector::new()),
        ];

        Self { event_tx, handlers }
    }
}

impl RtcStatsHandler for PeerMetricsService {
    /// Calls [`RtcStatsHandler::register_peer`] on all registered
    /// [`RtcStatsHandler`]s.
    fn register_peer(&mut self, peer: &PeerStateMachine) {
        for handler in &mut self.handlers {
            handler.register_peer(peer);
        }
    }

    /// Calls [`RtcStatsHandler::unregister_peers`] on the all registered
    /// [`RtcStatsHandler`]s.
    fn unregister_peers(&mut self, peers_ids: &[PeerId]) {
        for handler in &mut self.handlers {
            handler.unregister_peers(peers_ids);
        }
    }

    /// Calls [`RtcStatsHandler::update_peer`] on the all registered
    /// [`RtcStatsHandler`]s.
    fn update_peer(&mut self, peer: &PeerStateMachine) {
        for handler in &mut self.handlers {
            handler.update_peer(peer);
        }
    }

    /// Calls [`RtcStatsHandler::check`] on the all registered
    /// [`RtcStatsHandler`]s.
    fn check(&mut self) {
        for handler in &mut self.handlers {
            handler.check();
        }
    }

    /// Calls [`RtcStatsHandler::add_stats`] on the all registered
    /// [`RtcStatsHandler`]s.
    fn add_stats(&mut self, peer_id: PeerId, stats: &[RtcStat]) {
        for handler in &mut self.handlers {
            handler.add_stats(peer_id, stats);
        }
    }

    /// Calls [`RtcStatsHandler::update_peer_connection_state`] on the
    /// registered [`RtcStatsHandler`]s,
    fn update_peer_connection_state(
        &mut self,
        peer_id: PeerId,
        state: PeerConnectionState,
    ) {
        for handler in &mut self.handlers {
            handler.update_peer_connection_state(peer_id, state);
        }
    }

    /// Calls [`RtcStatsHandler::subscribe`] on the all registered
    /// [`RtcStatsHandler`]s returning merged stream.
    ///
    /// Creating new subscription will invalidate previous, so there may be only
    /// one subscription. Events are not saved or buffered at sending side, so
    /// you won't receive any events happened before subscription was made.
    fn subscribe(&mut self) -> LocalBoxStream<'static, PeersMetricsEvent> {
        stream::select_all(
            self.handlers.iter_mut().map(|handler| handler.subscribe()),
        )
        .boxed_local()
    }
}

/// [`PeersMetricsEvent`]s sender.
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
    /// If no one subscribed, then no-op.
    fn send_event(&self, event: PeersMetricsEvent) {
        if let Some(tx) = self.0.borrow().as_ref() {
            drop(tx.unbounded_send(event));
        }
    }

    /// Returns [`Stream`] of [`PeersMetricsEvent`]s.
    ///
    /// Creating new subscription will invalidate previous, so there may be only
    /// one subscription. Events are not saved or buffered at sending side, so
    /// you won't receive any events happened before subscription was made.
    ///
    /// [`Stream`]: futures::stream::Stream
    fn subscribe(&self) -> LocalBoxStream<'static, PeersMetricsEvent> {
        let (tx, rx) = mpsc::unbounded();
        self.0.borrow_mut().replace(tx);
        Box::pin(rx)
    }
}

//! [`ConnectionQualityScore`] score calculator implementation.

use std::{
    cell::RefCell,
    collections::HashMap,
    rc::{Rc, Weak},
    time::{Duration, SystemTime},
};

use futures::stream::LocalBoxStream;
use medea_client_api_proto::{
    stats::{
        RtcInboundRtpStreamStats, RtcRemoteInboundRtpStreamStats, RtcStat,
        RtcStatsType, StatId,
    },
    ConnectionQualityScore, MemberId, PeerConnectionState, PeerId,
};

use crate::{
    media::PeerStateMachine,
    signalling::peers::metrics::{
        EventSender, PeersMetricsEvent, RtcStatsHandler,
    },
};

/// [`RtcStatsHandler`] responsible for `Peer` connection quality estimation.
#[derive(Debug)]
pub(super) struct QualityMeterStatsHandler {
    /// All [`PeerMetric`]s registered in this [`QualityMeterStatsHandler`].
    peers: HashMap<PeerId, Rc<RefCell<PeerMetric>>>,

    /// [`PeersMetricsEvent`]s sender.
    event_tx: EventSender,
}

impl QualityMeterStatsHandler {
    /// Returns new empty [`QualityMeterStatsHandler`].
    pub(super) fn new() -> Self {
        Self {
            peers: HashMap::new(),
            event_tx: EventSender::new(),
        }
    }

    /// Recalculates [`ConnectionQualityScore`] for the provided
    /// [`PeerMetric`], sends [`PeersMetricsEvent::QualityMeterUpdate`] if
    /// new score is not equal to the previously calculated score.
    fn update_quality_score(&self, peer: &mut PeerMetric) {
        let partner_score = peer
            .partner_peer
            .upgrade()
            .and_then(|p| p.borrow_mut().calculate());
        let score = peer
            .calculate()
            .and_then(|score| {
                partner_score
                    .map(|partner_score| score.min(partner_score))
                    .or(Some(score))
            })
            .or(partner_score);

        if let Some(quality_score) = score {
            if quality_score == peer.last_quality_score {
                return;
            }

            peer.last_quality_score = quality_score;
            if let Some(partner_member_id) = peer.get_partner_member_id() {
                self.event_tx.send_event(
                    PeersMetricsEvent::QualityMeterUpdate {
                        member_id: peer.member_id.clone(),
                        partner_member_id,
                        quality_score,
                    },
                );
            }
        }
    }
}

impl RtcStatsHandler for QualityMeterStatsHandler {
    /// Creates [`PeerMetric`] for the provided [`PeerStateMachine`].
    ///
    /// Tries to add created [`PeerMetric`] to the partner [`PeerMetric`] if it
    /// exists.
    fn register_peer(&mut self, peer: &PeerStateMachine) {
        let id = peer.id();
        let partner_peer_id = peer.partner_peer_id();
        let partner_peer = self
            .peers
            .get(&partner_peer_id)
            .map(Rc::downgrade)
            .unwrap_or_default();
        let peer_metric = Rc::new(RefCell::new(PeerMetric {
            id,
            member_id: peer.member_id().clone(),
            partner_peer,
            quality_meter: QualityMeter::new(Duration::from_secs(5)),
            connection_state: PeerConnectionState::New,
            last_quality_score: ConnectionQualityScore::Poor,
        }));
        self.peers.insert(peer.id(), peer_metric.clone());

        if let Some(partner_peer) = self.peers.get(&partner_peer_id) {
            partner_peer.borrow_mut().partner_peer =
                Rc::downgrade(&peer_metric);
        }
    }

    /// Removes [`PeerMetric`]s with the provided [`PeerId`]s.
    fn unregister_peers(&mut self, peers_ids: &[PeerId]) {
        for peer_id in peers_ids {
            self.peers.remove(peer_id);
        }
    }

    /// Does nothing.
    fn update_peer(&mut self, _: &PeerStateMachine) {}

    /// Calculates new score for every registered `Peer`, sends
    /// [`PeersMetricsEvent::QualityMeterUpdate`] if new score is not equal
    /// to the previously calculated score.
    fn check(&mut self) {
        for peer in self.peers.values() {
            self.update_quality_score(&mut peer.borrow_mut());
        }
    }

    /// Tries to add provided [`RtcStat`]s to the [`QualityMeter`] of the
    /// [`PeerMetric`] with a provided [`PeerId`].
    ///
    /// Does nothing if [`PeerMetric`] with a provided [`PeerId`] not exists.
    fn add_stats(&mut self, peer_id: PeerId, stats: &[RtcStat]) {
        if let Some(peer) = self.peers.get(&peer_id) {
            let mut peer_ref = peer.borrow_mut();
            for stat in stats {
                match &stat.stats {
                    RtcStatsType::InboundRtp(inbound) => {
                        if let Some(partner_peer) =
                            peer_ref.partner_peer.upgrade()
                        {
                            partner_peer
                                .borrow_mut()
                                .add_outbound_from_partners_inbound(
                                    stat.id.clone(),
                                    inbound,
                                );
                        }
                    }
                    RtcStatsType::RemoteInboundRtp(remote_inbound) => {
                        peer_ref.add_remote_inbound_rtp(remote_inbound);
                    }
                    _ => (),
                }
            }
        }
    }

    /// Updates `Peer`s [`PeerConnectionState`], recalculates
    /// [`ConnectionQualityScore`] for specified `Peer`.
    ///
    /// Does nothing if [`PeerMetric`] with a provided [`PeerId`] not exists.
    #[inline]
    fn update_peer_connection_state(
        &mut self,
        peer_id: PeerId,
        connection_state: PeerConnectionState,
    ) {
        if let Some(peer) = self.peers.get(&peer_id) {
            peer.borrow_mut().connection_state = connection_state;
            self.update_quality_score(&mut peer.borrow_mut());
        }
    }

    fn subscribe(&mut self) -> LocalBoxStream<'static, PeersMetricsEvent> {
        self.event_tx.subscribe()
    }
}

/// [`PeerStateMachine`] representation for the [`QualityMeterStatsHandler`].
#[derive(Debug)]
struct PeerMetric {
    /// ID of [`PeerStateMachine`] for which this [`PeerMetric`] was created.
    id: PeerId,

    /// [`MemberId`] of the [`Member`] which owns this [`Peer`].
    ///
    /// [`Member`]: crate::signalling::elements::Member
    /// [`Peer`]: crate::media::peer::Peer
    member_id: MemberId,

    /// Weak reference to a [`PeerMetric`] which represents a partner
    /// [`PeerStateMachine`].
    partner_peer: Weak<RefCell<PeerMetric>>,

    /// [`ConnectionQualityScore`] score calculator for this [`PeerMetric`].
    quality_meter: QualityMeter,

    /// Last calculated [`ConnectionQualityScore`].
    last_quality_score: ConnectionQualityScore,

    /// Current [`PeerConnectionState`].
    connection_state: PeerConnectionState,
}

impl PeerMetric {
    /// Adds rtt and jitter stats from provided
    /// [`RtcRemoteInboundRtpStreamStats`] stats to the [`QualityMeter`].
    fn add_remote_inbound_rtp(&mut self, upd: &RtcRemoteInboundRtpStreamStats) {
        if let Some(jitter) = upd.jitter.map(|f| f.0).filter(|j| *j > 0.) {
            self.quality_meter
                .add_jitter(Duration::from_secs_f64(jitter));
        }
        if let Some(rtt) = upd.round_trip_time.map(|f| f.0).filter(|t| *t > 0.)
        {
            self.quality_meter.add_rtt(Duration::from_secs_f64(rtt));
        }
    }

    /// Adds packets lost and packets sent stats from provided partners
    /// [`RtcInboundRtpStreamStats`] stats to the [`QualityMeter`].
    fn add_outbound_from_partners_inbound(
        &mut self,
        stat_id: StatId,
        upd: &RtcInboundRtpStreamStats,
    ) {
        #[allow(clippy::cast_sign_loss)]
        let packets_lost =
            upd.packets_lost.map_or(0, |plost| plost.max(0)) as u64;
        self.quality_meter
            .add_packets_lost(stat_id.clone(), packets_lost);
        self.quality_meter
            .add_packets_sent(stat_id, upd.packets_received + packets_lost);
    }

    /// Returns [`MemberId`] of the partner [`Member`].
    ///
    /// [`Member`]: crate::signalling::elements::Member
    fn get_partner_member_id(&self) -> Option<MemberId> {
        self.partner_peer
            .upgrade()
            .map(|partner_peer| partner_peer.borrow().member_id.clone())
    }

    /// Calculates current [`ConnectionQualityScore`] based on the current
    /// connection state and [`QualityMeter`] estimation.
    fn calculate(&mut self) -> Option<ConnectionQualityScore> {
        self.calculate_from_connection_state()
            .or_else(|| self.quality_meter.calculate())
    }

    /// Calculates [`ConnectionQualityScore`] based on the
    /// current connection state.
    fn calculate_from_connection_state(
        &self,
    ) -> Option<ConnectionQualityScore> {
        match self.connection_state {
            PeerConnectionState::Connected => None,
            _ => Some(ConnectionQualityScore::Poor),
        }
    }
}

/// Calculator of the [`ConnectionQualityScore`] score based on RTC stats.
#[derive(Debug)]
struct QualityMeter {
    /// TTL of the all [`ExpiringStat`]s from this [`QualityMeter`].
    stats_ttl: Duration,

    /// Round trip time stats.
    ///
    /// Expired values will be automatically removed.
    rtt: Vec<ExpiringStat<Rtt>>,

    /// All jitter values added to this [`QualityMeter`].
    ///
    /// Expired values will be automatically removed.
    jitter: Vec<ExpiringStat<Jitter>>,

    /// Packets lost stats by [`StatId`].
    ///
    /// Expired stats will be automatically removed.
    packets_lost: HashMap<StatId, Vec<ExpiringStat<PacketLost>>>,

    /// Packets sent stats by [`StatId`].
    ///
    /// Expired stats will be automatically removed.
    packets_sent: HashMap<StatId, Vec<ExpiringStat<PacketsSent>>>,
}

impl QualityMeter {
    /// Jitter multiplier used in effective latency calculation.
    const JITTER_FACTOR: f64 = 2.5;
    /// Latency multiplier used in effective latency calculation.
    const LATENCY_FACTOR: f64 = 0.7;
    /// Estimated packet loss multiplier.
    const P_LOSS_FACTOR: f64 = 2.5;
    /// `R0` is the basic signal to noise ratio, including noise sources such
    /// as circuit and room noise. However, currently it is really difficult
    /// to calculate directly. Thus, [ITU-T G.113] provides the common value.
    ///
    /// [ITU-T G.113]: https://itu.int/rec/T-REC-G.113
    const R0: f64 = 93.2;
    /// Relationship between R-value and user's satisfaction is taken from
    /// [ITU-T G.107].
    ///
    /// [ITU-T G.107]: https://itu.int/rec/T-REC-G.107
    const R_LOWER_LIMIT_HIGH: f64 = 80.;
    const R_LOWER_LIMIT_LOW: f64 = 60.;
    const R_LOWER_LIMIT_MEDIUM: f64 = 70.;

    /// Returns new empty [`QualityMeter`].
    ///
    /// Provided stats TTL will be used to decide when [`ExpiringStat`] should
    /// expire.
    fn new(stats_ttl: Duration) -> Self {
        Self {
            stats_ttl,
            rtt: Vec::new(),
            jitter: Vec::new(),
            packets_lost: HashMap::new(),
            packets_sent: HashMap::new(),
        }
    }

    /// Adds new round trip time measurement.
    fn add_rtt(&mut self, rtt: Duration) {
        self.rtt.push(ExpiringStat::new(Rtt(rtt), self.stats_ttl));
    }

    /// Adds new jitter measurement.
    fn add_jitter(&mut self, jitter: Duration) {
        self.jitter
            .push(ExpiringStat::new(Jitter(jitter), self.stats_ttl));
    }

    /// Adds new packets sent measurement.
    fn add_packets_sent(&mut self, stat_id: StatId, packets_sent: u64) {
        self.packets_sent
            .entry(stat_id)
            .or_default()
            .push(ExpiringStat::new(PacketsSent(packets_sent), self.stats_ttl));
    }

    /// Adds new packets lost measurement.
    fn add_packets_lost(&mut self, stat_id: StatId, packets_lost: u64) {
        self.packets_lost
            .entry(stat_id)
            .or_default()
            .push(ExpiringStat::new(PacketLost(packets_lost), self.stats_ttl));
    }

    /// Returns [`ConnectionQualityScore`] based on accumulated stats.
    /// Returns `None` if there are not enough data to make calculations.
    ///
    /// [Algorithm-MOS] is used to calculate [`ConnectionQualityScore`], which
    /// is derived from E-model, introduced in [ITU-T G.107] with some
    /// simplifications and tweaks.
    ///
    /// [ITU-T G.107]: https://itu.int/rec/T-REC-G.107
    /// [Algorithm-MOS]: https://tinyurl.com/y3nojmot
    #[allow(clippy::cast_precision_loss)]
    fn calculate(&mut self) -> Option<ConnectionQualityScore> {
        let latency = self.mean_rtt()?.as_millis() as f64;
        let jitter = self.mean_jitter()?.as_millis() as f64;
        let packet_loss = self.mean_packet_loss()?;

        let effective_latency =
            jitter * Self::JITTER_FACTOR + latency * Self::LATENCY_FACTOR;

        // Calculate the R-Value (Transmission Rating Factor R) based on
        // Effective Latency. The voice quality drops more significantly
        // over 160ms so the R-Value is penalized more.
        let r = if effective_latency < 160. {
            Self::R0 - (effective_latency / 40.)
        } else {
            Self::R0 - (effective_latency - 120.) / 10.
        };

        let r = r - (packet_loss * Self::P_LOSS_FACTOR);
        {
            use ConnectionQualityScore::{High, Low, Medium, Poor};

            Some(if r < Self::R_LOWER_LIMIT_LOW {
                Poor
            } else if r < Self::R_LOWER_LIMIT_MEDIUM {
                Low
            } else if r < Self::R_LOWER_LIMIT_HIGH {
                Medium
            } else {
                High
            })
        }
    }

    /// Returns average round trip time based on accumulated [`Rtt`] stats stats
    /// filtering out expired measurements.
    ///
    /// Returns `None` if there are not enough data to make calculations.
    fn mean_rtt(&mut self) -> Option<Duration> {
        remove_expired_stats(&mut self.rtt);
        if self.rtt.is_empty() {
            None
        } else {
            #[allow(clippy::cast_possible_truncation)]
            Some(
                self.rtt.iter().map(|s| s.stat.0).sum::<Duration>()
                    / (self.rtt.len() as u32),
            )
        }
    }

    /// Returns average jitter based on accumulated [`Jitter`] stats stats
    /// filtering out expired measurements.
    ///
    /// Returns `None` if there are not enough data to make calculations.
    fn mean_jitter(&mut self) -> Option<Duration> {
        remove_expired_stats(&mut self.jitter);
        if self.jitter.is_empty() {
            None
        } else {
            #[allow(clippy::cast_possible_truncation)]
            Some(
                self.jitter.iter().map(|s| s.stat.0).sum::<Duration>()
                    / self.jitter.len() as u32,
            )
        }
    }

    /// Returns average packet loss based on accumulated [`PacketLost`] and
    /// [`PacketsSent`] stats filtering out expired measurements.
    ///
    /// Returns `None` if there are not enough data to make calculations.
    fn mean_packet_loss(&mut self) -> Option<f64> {
        self.packets_lost.retain(|_, row| {
            remove_expired_stats(row);
            !row.is_empty()
        });
        self.packets_sent.retain(|_, row| {
            remove_expired_stats(row);
            !row.is_empty()
        });

        let mut sum_lost_delta = 0;
        let mut sum_sent_delta = 0;
        for (ssrc, lost) in &self.packets_lost {
            let sent = self.packets_sent.get(ssrc)?;

            let min_lost = lost.iter().map(|s| s.stat.0).min()?;
            let max_lost = lost.iter().map(|s| s.stat.0).max()?;
            let min_sent = sent.iter().map(|s| s.stat.0).min()?;
            let max_sent = sent.iter().map(|s| s.stat.0).max()?;

            sum_lost_delta += max_lost - min_lost;
            sum_sent_delta += max_sent - min_sent;
        }

        if sum_sent_delta == 0 {
            Some(0.)
        } else if sum_lost_delta > sum_sent_delta {
            Some(100.)
        } else {
            #[allow(clippy::cast_precision_loss)]
            Some((sum_lost_delta as f64 * 100.) / sum_sent_delta as f64)
        }
    }
}

/// Retains expired [`ExpiringStat`]s from the `Vec<ExpiringStat<T>` storage.
///
/// Expiration will be considered by calling [`ExpiringStat::is_expired`].
fn remove_expired_stats<T>(stats: &mut Vec<ExpiringStat<T>>) {
    stats.retain(|s| !s.is_expired());
}

/// Wrapper around stat which implements expiration logic.
///
/// Periodically, storage of the [`ExpiringStat`]s should check all stored
/// values by calling [`ExpiringStat::is_expired`] and if it returns `true`,
/// remove this stat from the storage.
#[derive(Debug)]
struct ExpiringStat<T> {
    /// Timestamp when this [`ExpiringStat`] was measured.
    measured_at: SystemTime,

    /// TTL (time to live) for this [`ExpiringStat`].
    ttl: Duration,

    /// Actual value of this [`ExpiringStat`].
    stat: T,
}

impl<T> ExpiringStat<T> {
    /// Creates new [`ExpiringStat`] with a provided TTL.
    ///
    /// This [`ExpiringStat`] will be considered as expired after provided
    /// [`Duration`].
    fn new(stat: T, ttl: Duration) -> Self {
        Self {
            measured_at: SystemTime::now(),
            ttl,
            stat,
        }
    }

    /// Indicates whether this [`ExpiringStat`] was considered as expired and
    /// should be removed from the storage.
    fn is_expired(&self) -> bool {
        self.measured_at.elapsed().unwrap() > self.ttl
    }
}

/// Estimated round trip time for specific SSRC measured in milliseconds
#[derive(Debug)]
struct Rtt(Duration);

/// Packet jitter for specific SSRC measured in milliseconds.
///
/// [jitter]: https://en.wikipedia.org/wiki/Jitter
#[derive(Debug)]
struct Jitter(Duration);

/// Accumulated number of packets lost for specific SSRC.
#[derive(Debug)]
struct PacketLost(u64);

/// Accumulated number of packets sent for specific SSRC.
#[derive(Debug)]
struct PacketsSent(u64);

#[cfg(test)]
mod tests {
    use futures::StreamExt as _;
    use medea_client_api_proto::stats::{
        Float, HighResTimeStamp, RtcInboundRtpStreamMediaType,
    };

    use crate::media::{peer::MockPeerUpdatesSubscriber, Peer};

    use super::*;

    const STATS_TTL: Duration = Duration::from_secs(5);

    #[test]
    fn packets_lost() {
        let mut meter = QualityMeter::new(STATS_TTL);
        meter.add_packets_sent(StatId::from("audio"), 100);
        assert_eq!(meter.mean_packet_loss(), Some(0.));

        meter.add_packets_lost(StatId::from("audio"), 33);
        assert_eq!(meter.mean_packet_loss(), Some(0.));

        meter.add_packets_sent(StatId::from("audio"), 100);
        assert_eq!(meter.mean_packet_loss(), Some(0.));

        meter.add_packets_lost(StatId::from("audio"), 66);
        assert_eq!(meter.mean_packet_loss(), Some(0.));

        meter.add_packets_sent(StatId::from("audio"), 500);
        assert_eq!(meter.mean_packet_loss(), Some(33. * 100. / (400.)));

        meter.add_packets_sent(StatId::from("video"), 0);
        meter.add_packets_sent(StatId::from("video"), 500);
        meter.add_packets_lost(StatId::from("video"), 0);
        assert_eq!(meter.mean_packet_loss(), Some(33. * 100. / (900.)));

        meter.add_packets_lost(StatId::from("video"), 33);
        assert_eq!(meter.mean_packet_loss(), Some(66. * 100. / (900.)));

        meter.add_packets_lost(StatId::from("audio"), 133);
        assert_eq!(meter.mean_packet_loss(), Some(133. * 100. / (900.)));

        meter.add_packets_sent(StatId::from("video"), 1000);
        assert_eq!(meter.mean_packet_loss(), Some(133. * 100. / (1400.)));
    }

    #[test]
    fn very_good_call_quality() {
        let mut meter = QualityMeter::new(STATS_TTL);
        meter.add_packets_lost(StatId::from("111"), 0);
        meter.add_packets_sent(StatId::from("111"), 1000);
        meter.add_rtt(Duration::from_millis(0));
        for jitter in &[0, 0, 0] {
            meter.add_jitter(Duration::from_millis(*jitter));
        }

        assert_eq!(meter.calculate().unwrap(), ConnectionQualityScore::High);
    }

    #[test]
    fn regular_normal_call() {
        let mut meter = QualityMeter::new(STATS_TTL);

        for jitter in &[0, 10, 12, 10] {
            meter.add_jitter(Duration::from_millis(*jitter));
        }
        for (packet_lost, packets_received) in &[
            (0, 45),
            (0, 50),
            (0, 95),
            (0, 96),
            (0, 146),
            (0, 158),
            (0, 197),
        ] {
            meter.add_packets_lost(StatId::from("a"), *packet_lost);
            meter.add_packets_sent(StatId::from("a"), *packets_received);
        }
        for rtt in &[20, 30, 20, 30] {
            meter.add_rtt(Duration::from_millis(*rtt));
        }

        assert_eq!(meter.calculate().unwrap(), ConnectionQualityScore::High);
    }

    #[test]
    fn bad_call() {
        let mut meter = QualityMeter::new(STATS_TTL);

        for jitter in &[10, 20, 15, 16, 11] {
            meter.add_jitter(Duration::from_millis(*jitter));
        }
        for (packet_lost, packets_sent) in &[
            (3, 45),
            (6, 50),
            (7, 95),
            (7, 96),
            (11, 146),
            (12, 158),
            (15, 197),
            (19, 217),
        ] {
            meter.add_packets_lost(StatId::from("a"), *packet_lost);
            meter.add_packets_sent(StatId::from("a"), *packets_sent);
        }
        for rtt in &[150, 160, 170, 150] {
            meter.add_rtt(Duration::from_millis(*rtt));
        }

        assert_eq!(meter.calculate().unwrap(), ConnectionQualityScore::Low);
    }

    #[test]
    fn extremely_bad_call() {
        let mut meter = QualityMeter::new(STATS_TTL);
        meter.add_packets_lost(StatId::from("a"), 100);
        meter.add_packets_sent(StatId::from("a"), 100);
        meter.add_rtt(Duration::from_millis(1000));
        for jitter in &[10, 1000, 3000] {
            meter.add_jitter(Duration::from_millis(*jitter));
        }

        assert_eq!(meter.calculate().unwrap(), ConnectionQualityScore::Poor);
    }

    #[test]
    fn rtt_and_jitter_stats_expire() {
        let expired = SystemTime::now() - Duration::from_secs(6);

        let mut meter = QualityMeter::new(STATS_TTL);

        meter.add_rtt(Duration::from_millis(0));
        meter.add_jitter(Duration::from_millis(0));

        meter.rtt.get_mut(0).unwrap().measured_at = expired;
        meter.jitter.get_mut(0).unwrap().measured_at = expired;

        meter.add_rtt(Duration::from_millis(0));
        meter.add_jitter(Duration::from_millis(0));

        assert_eq!(meter.rtt.len(), 2);
        assert_eq!(meter.jitter.len(), 2);

        meter.calculate();

        assert_eq!(meter.rtt.len(), 1);
        assert_eq!(meter.jitter.len(), 1);
    }

    #[test]
    fn psent_and_plost_stats_expire() {
        let expired = SystemTime::now() - Duration::from_secs(6);

        let mut meter = QualityMeter::new(STATS_TTL);

        meter.add_rtt(Duration::from_millis(0));
        meter.add_jitter(Duration::from_millis(0));

        meter.add_packets_sent(StatId::from("a"), 100);
        meter.add_packets_lost(StatId::from("a"), 20);

        meter
            .packets_sent
            .get_mut(&StatId::from("a"))
            .unwrap()
            .get_mut(0)
            .unwrap()
            .measured_at = expired;
        meter
            .packets_lost
            .get_mut(&StatId::from("a"))
            .unwrap()
            .get_mut(0)
            .unwrap()
            .measured_at = expired;

        meter.add_packets_sent(StatId::from("a"), 200);
        meter.add_packets_lost(StatId::from("a"), 40);

        meter.calculate();

        assert_eq!(meter.rtt.len(), 1);
        assert_eq!(meter.jitter.len(), 1);
        assert_eq!(meter.packets_sent.len(), 1);
        assert_eq!(
            meter.packets_sent.get(&StatId::from("a")).unwrap().len(),
            1
        );
        assert_eq!(meter.packets_lost.len(), 1);
        assert_eq!(
            meter.packets_lost.get(&StatId::from("a")).unwrap().len(),
            1
        );

        // make sure that entries are completely removed if values are empty
        meter
            .packets_sent
            .get_mut(&StatId::from("a"))
            .unwrap()
            .get_mut(0)
            .unwrap()
            .measured_at = expired;
        meter
            .packets_lost
            .get_mut(&StatId::from("a"))
            .unwrap()
            .get_mut(0)
            .unwrap()
            .measured_at = expired;

        meter.calculate();

        assert_eq!(meter.packets_sent.len(), 0);
        assert_eq!(meter.packets_lost.len(), 0);
    }

    #[tokio::test]
    async fn connection_state() {
        let mut stats_handler = QualityMeterStatsHandler::new();
        let metrics_events = stats_handler.subscribe();
        let member_id = MemberId::from("member-1");
        let partner_member_id = MemberId::from("member-1");

        let peer1: PeerStateMachine = Peer::new(
            PeerId(0),
            member_id.clone(),
            PeerId(1),
            partner_member_id.clone(),
            false,
            Rc::new(MockPeerUpdatesSubscriber::new()),
        )
        .into();
        let peer2: PeerStateMachine = Peer::new(
            PeerId(1),
            partner_member_id.clone(),
            PeerId(0),
            member_id.clone(),
            false,
            Rc::new(MockPeerUpdatesSubscriber::new()),
        )
        .into();

        stats_handler.register_peer(&peer1);
        stats_handler.register_peer(&peer2);
        stats_handler.add_stats(
            PeerId(1),
            &[RtcStat {
                id: StatId::from("InboundRtp"),
                timestamp: HighResTimeStamp(0.),
                stats: RtcStatsType::InboundRtp(Box::new(
                    RtcInboundRtpStreamStats {
                        track_id: None,
                        media_specific_stats:
                            RtcInboundRtpStreamMediaType::Audio {
                                voice_activity_flag: None,
                                total_samples_received: None,
                                concealed_samples: None,
                                silent_concealed_samples: None,
                                audio_level: None,
                                total_audio_energy: None,
                                total_samples_duration: None,
                            },
                        bytes_received: 0,
                        packets_received: 100,
                        packets_lost: Some(0),
                        jitter: None,
                        total_decode_time: None,
                        jitter_buffer_emitted_count: None,
                    },
                )),
            }],
        );
        stats_handler.add_stats(
            PeerId(0),
            &[RtcStat {
                id: StatId::from("RemoteInboundRtp"),
                timestamp: HighResTimeStamp(0.),
                stats: RtcStatsType::RemoteInboundRtp(Box::new(
                    RtcRemoteInboundRtpStreamStats {
                        local_id: None,
                        jitter: Some(Float(0.01)),
                        round_trip_time: Some(Float(0.01)),
                        fraction_lost: None,
                        reports_received: None,
                        round_trip_time_measurements: None,
                    },
                )),
            }],
        );
        stats_handler.check();
        stats_handler.update_peer_connection_state(
            PeerId(0),
            PeerConnectionState::Connecting,
        );
        stats_handler.update_peer_connection_state(
            PeerId(1),
            PeerConnectionState::Connecting,
        );
        stats_handler.update_peer_connection_state(
            PeerId(0),
            PeerConnectionState::Connected,
        );
        stats_handler.update_peer_connection_state(
            PeerId(1),
            PeerConnectionState::Connected,
        );
        stats_handler.update_peer_connection_state(
            PeerId(0),
            PeerConnectionState::Disconnected,
        );
        stats_handler.update_peer_connection_state(
            PeerId(1),
            PeerConnectionState::Disconnected,
        );
        stats_handler.update_peer_connection_state(
            PeerId(0),
            PeerConnectionState::Connected,
        );
        stats_handler.update_peer_connection_state(
            PeerId(1),
            PeerConnectionState::Connected,
        );
        stats_handler.update_peer_connection_state(
            PeerId(1),
            PeerConnectionState::Failed,
        );
        drop(stats_handler);

        let high = PeersMetricsEvent::QualityMeterUpdate {
            member_id: partner_member_id.clone(),
            partner_member_id: member_id.clone(),
            quality_score: ConnectionQualityScore::High,
        };
        let poor = PeersMetricsEvent::QualityMeterUpdate {
            member_id: partner_member_id,
            partner_member_id: member_id,
            quality_score: ConnectionQualityScore::Poor,
        };
        let events: Vec<_> = metrics_events.collect().await;
        assert_eq!(events, &[high.clone(), poor.clone(), high, poor]);
    }
}

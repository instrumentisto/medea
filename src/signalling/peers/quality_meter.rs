//! [`ConnectionQualityScore`] score calculator implementation.

use std::{
    collections::HashMap,
    time::{Duration, SystemTime},
};

use medea_client_api_proto::{stats::StatId, ConnectionQualityScore};

/// Calculator of the [`ConnectionQualityScore`] score based on RTC stats.
#[derive(Debug)]
pub struct QualityMeter {
    /// TTL of the all [`ExpiringStat`]s from this [`QualityMeter`].
    stats_ttl: Duration,

    /// Round trip time stats.
    rtt: Vec<ExpiringStat<Rtt>>,

    /// All jitter values added to this [`QualityMeter`].
    ///
    /// Expired stats will be automatically removed.
    jitter: Vec<ExpiringStat<Jitter>>,

    /// Stores packets lost stats separated by [`StatId`].
    ///
    /// Expired stats will be automatically removed.
    packets_lost: HashMap<StatId, Vec<ExpiringStat<PacketLost>>>,

    /// Stores packets sent stats separated by [`StatId`].
    ///
    /// Expired stats will be automatically removed.
    packets_sent: HashMap<StatId, Vec<ExpiringStat<PacketsSent>>>,
}

impl QualityMeter {
    /// Estimated delay introduced by codec used.
    const CODEC_DELAY: f64 = 10.;
    /// Jitter multiplier used in effective latency calculation.
    const JITTER_FACTOR: f64 = 2.;
    /// Estimated packet loss multiplier.
    const P_LOSS_FACTOR: f64 = 2.5;
    /// `R0` is the basic signal to noise ratio, including noise sources such
    /// as circuit and room noise. However, currently it is really difficult
    /// to calculate directly. Thus, [ITU-T G.113] provides the common value.
    ///
    ///  [ITU-T G.113]: https://www.itu.int/rec/T-REC-G.113
    const R0: f64 = 93.2;
    /// Relationship between R-value and user's satisfaction is taken from
    /// [ITU-T G.107].
    ///
    /// [ITU-T G.107]: https://www.itu.int/rec/T-REC-G.107
    const R_LOWER_LIMIT_HIGH: f64 = 80.;
    const R_LOWER_LIMIT_LOW: f64 = 60.;
    const R_LOWER_LIMIT_MEDIUM: f64 = 70.;

    /// Returns new empty [`QualityMeter`].
    ///
    /// Provided stats TTL will be used to decide when [`ExpiringStat`] should
    /// expire.
    pub fn new(stats_ttl: Duration) -> Self {
        Self {
            stats_ttl,
            rtt: Vec::new(),
            jitter: Vec::new(),
            packets_lost: HashMap::new(),
            packets_sent: HashMap::new(),
        }
    }

    /// Adds new round trip time measurement.
    pub fn add_rtt(&mut self, rtt: Duration) {
        self.rtt.push(ExpiringStat::new(Rtt(rtt), self.stats_ttl));
    }

    /// Adds new jitter measurement.
    pub fn add_jitter(&mut self, jitter: Duration) {
        self.jitter
            .push(ExpiringStat::new(Jitter(jitter), self.stats_ttl));
    }

    /// Adds new packets sent measurement.
    pub fn add_packets_sent(&mut self, stat_id: StatId, packets_sent: u64) {
        self.packets_sent
            .entry(stat_id)
            .or_default()
            .push(ExpiringStat::new(PacketsSent(packets_sent), self.stats_ttl));
    }

    /// Adds new packets lost measurement.
    pub fn add_packets_lost(&mut self, stat_id: StatId, packets_lost: u64) {
        self.packets_lost
            .entry(stat_id)
            .or_default()
            .push(ExpiringStat::new(PacketLost(packets_lost), self.stats_ttl));
    }

    /// Returns [`ConnectionQualityScore`] based on accumulated stats.
    /// Returns `None` if there are not enough data to make calculations.
    ///
    /// [Algorithm-MOS] is used to calculate [`ConnectionQualityScore`],
    /// which is derived from E-model, introduced in [ITU-T G.107] with some
    /// simplifications and tweaks.
    ///
    /// [ITU-T G.107]: https://www.itu.int/rec/T-REC-G.107
    /// [Algorithm-MOS]: https://tinyurl.com/y3nojmot
    #[allow(clippy::cast_precision_loss)]
    pub fn calculate(&mut self) -> Option<ConnectionQualityScore> {
        let latency = self.mean_rtt()?.as_millis() as f64;
        let jitter = self.mean_jitter()?.as_millis() as f64;
        let packet_loss = self.mean_packet_loss()?;

        let effective_latency =
            jitter * Self::JITTER_FACTOR + latency + Self::CODEC_DELAY;

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
    /// filtering out expired measurements. Returns `None` if there are not
    /// enough data to make calculations.
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
    /// filtering out expired measurements. Returns `None` if there are not
    /// enough data to make calculations.
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
/// Periodically storage of the [`ExpiringStat`]s should check all stored values
/// by calling [`ExpiringStat::is_expired`] and if it returns `true`, remove
/// this stat from the storage.
#[derive(Debug)]
struct ExpiringStat<T> {
    /// Timestamp when stat was measured.
    measured_at: SystemTime,
    /// Stat TTL.
    ttl: Duration,
    /// Actual stat.
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

    /// Returns `true` if this [`ExpiringStat`] was considered as expired and
    /// should be removed from the storage.
    fn is_expired(&self) -> bool {
        self.measured_at.elapsed().unwrap() > self.ttl
    }
}

/// Estimated round trip time for specific SSRC measured in milliseconds
#[derive(Debug)]
struct Rtt(Duration);

/// Packet Jitter for specific SSRC measured in milliseconds.
#[derive(Debug)]
struct Jitter(Duration);

/// Accumulative number of packets lost for specific SSRC.
#[derive(Debug)]
struct PacketLost(u64);

/// Accumulative number of packets sent for specific SSRC.
#[derive(Debug)]
struct PacketsSent(u64);

#[cfg(test)]
mod tests {
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
}

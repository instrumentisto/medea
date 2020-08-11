use std::{
    collections::HashMap,
    convert::TryFrom,
    time::{Duration, SystemTime},
};

use derive_more::Display;
use medea_client_api_proto::stats::StatId;

/// Burns outdated stats from the provided [`Vec`].
fn burn<T>(stats: &mut Vec<BurningStat<T>>) {
    let burned_stats = std::mem::replace(stats, Vec::new());

    *stats = burned_stats
        .into_iter()
        .filter(|s| !s.should_be_burned())
        .collect();
}

/// Estimated connection quality.
#[derive(Clone, Debug, Copy, PartialEq, Eq, Display)]
pub enum EstimatedConnectionQuality {
    /// All users are dissatisfied.
    AllDissatisfied = 1,

    /// Many users are dissatisfied.
    ManyDissatisfied = 2,

    /// Some users are dissatisfied.
    SomeDissatisfied = 3,

    /// All users are satisfied.
    Satisfied = 4,
}

impl TryFrom<u8> for EstimatedConnectionQuality {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            1 => Self::AllDissatisfied,
            2 => Self::ManyDissatisfied,
            3 => Self::SomeDissatisfied,
            4 => Self::Satisfied,
            _ => return Err(()),
        })
    }
}

impl EstimatedConnectionQuality {
    /// Returns average [`EstimatedConnectionQuality`] between two
    /// [`EstimatedConnectionQuality`].
    pub fn avg(first: Self, second: Self) -> Self {
        // Should never panic.
        Self::try_from(((first as u8) + (second as u8)) / 2).unwrap()
    }
}

/// Calculator of the [`EstimatedConnectionQuality`] score based on RTC stats.
#[derive(Debug)]
pub struct QualityMeter {
    /// Round trip time stats.
    rtt: Vec<BurningStat<Rtt>>,

    /// Jitter stats.
    ///
    /// This kind of stats are separated by [`StatId`], but will be united on
    /// calculation phase.
    jitter: HashMap<StatId, Vec<BurningStat<Jitter>>>,

    /// Packet loss stats in percents (0..100).
    ///
    /// This kind of stats are separated by [`StatId`], but will be united on
    /// calculation phase.
    ///
    /// Calculated based on `packetsReceived` and `packetsLost`.
    packet_loss: HashMap<StatId, Vec<BurningStat<PacketLoss>>>,

    /// Last packets lost count for the all [`StatId`]s.
    last_packets_lost: HashMap<StatId, u64>,

    /// Last total packets count for the all [`StatId`]s.
    last_total_packets: HashMap<StatId, u64>,
}

impl QualityMeter {
    const CODEC_DELAY: f64 = 10.0;
    const JITTER_FACTOR: f64 = 2.0;
    const P_LOSS_FACTOR: f64 = 2.5;
    const R0: f64 = 93.2;
    const R_LOWER_LIMIT_ALL_DISSATISFIED: f64 = 50.0;
    const R_LOWER_LIMIT_MANY_DISSATISFIED: f64 = 60.0;
    const R_LOWER_LIMIT_SOME_DISSATISFIED: f64 = 70.0;

    /// Returns new empty [`QualityMeter`].
    pub fn new() -> Self {
        Self {
            rtt: Vec::new(),
            jitter: HashMap::new(),
            packet_loss: HashMap::new(),
            last_packets_lost: HashMap::new(),
            last_total_packets: HashMap::new(),
        }
    }

    /// Adds new round trip time measurement.
    pub fn add_rtt(&mut self, timestamp: SystemTime, rtt: u64) {
        self.rtt.push(BurningStat::new(Rtt(rtt), timestamp));
    }

    /// Adds new jitter measurement.
    pub fn add_jitter(
        &mut self,
        timestamp: SystemTime,
        stat_id: StatId,
        jitter: u64,
    ) {
        self.jitter
            .entry(stat_id)
            .or_default()
            .push(BurningStat::new(Jitter(jitter), timestamp));
    }

    /// Adds packet loss stat based on the provided `packetsLost` and
    /// `packetsReceived` stats.
    pub fn add_packet_loss(
        &mut self,
        timestamp: SystemTime,
        stat_id: StatId,
        packets_lost: u64,
        packets_received: u64,
    ) {
        let total_packets = packets_received + packets_lost;
        let last_packets_lost = if let Some(last_packets_lost) =
            self.last_packets_lost.get(&stat_id)
        {
            if *last_packets_lost > packets_lost {
                return;
            } else {
                *last_packets_lost
            }
        } else {
            0
        };
        let last_total_packets = if let Some(last_total_packets) =
            self.last_total_packets.get(&stat_id)
        {
            if *last_total_packets > total_packets {
                return;
            } else {
                *last_total_packets
            }
        } else {
            0
        };

        let packet_loss_at_period = packets_lost - last_packets_lost;
        let total_packets_at_period = total_packets - last_total_packets;
        self.last_total_packets
            .insert(stat_id.clone(), total_packets);
        self.last_packets_lost.insert(stat_id.clone(), packets_lost);
        if total_packets_at_period == 0 {
            return;
        }

        let packet_loss =
            (packet_loss_at_period as f64) / (total_packets_at_period as f64);

        self.packet_loss
            .entry(stat_id)
            .or_default()
            .push(BurningStat::new(
                PacketLoss((packet_loss * 100.0) as u64),
                timestamp,
            ));
    }

    /// Burns all outdated stats from this [`QualityMeter`].
    fn burn_stats(&mut self) {
        burn(&mut self.rtt);
        self.jitter.values_mut().for_each(burn);
        self.packet_loss.values_mut().for_each(burn);
    }

    /// Returns average round trip time.
    ///
    /// Returns `None` if [`QualityMeter`] doesn't have any round trip time
    /// stats.
    fn average_latency(&self) -> Option<f64> {
        if self.rtt.is_empty() {
            return None;
        }

        Some(
            self.rtt.iter().map(|s| s.stat.0 as f64).sum::<f64>()
                / (self.rtt.len() as f64),
        )
    }

    /// Returns average jitter value based on the all jitter stats from
    /// [`QualityMeter`].
    ///
    /// Returns `None` if [`QualityMeter`] doesn't have any jitter stats.
    fn jitter(&self) -> Option<u64> {
        let jitter: Vec<u64> = self
            .jitter
            .values()
            .filter_map(|jitter| {
                let mut jitter_iter = jitter.iter();
                let mut prev = jitter_iter.next().map(|s| s.stat.0)?;
                Some(jitter_iter.map(|j| j.stat.0).fold(0, |acc, val| {
                    let out = if prev > val {
                        acc + prev - val
                    } else {
                        acc + val - prev
                    };
                    prev = val;

                    out
                }))
            })
            .collect();

        let count = jitter.len();
        if count < 1 {
            return None;
        }

        Some(jitter.into_iter().sum::<u64>() / count as u64)
    }

    /// Returns average packet loss based on the al packet loss stats from
    /// [`QualityMeter`].
    ///
    /// Returns `None` if [`QualityMeter`] doesn't have any packet loss stats.
    fn average_packet_loss(&self) -> Option<f64> {
        let packet_loss: Vec<f64> = self
            .packet_loss
            .values()
            .filter_map(|packet_loss| {
                if packet_loss.is_empty() {
                    return None;
                }

                Some(
                    packet_loss.iter().map(|s| s.stat.0 as f64).sum::<f64>()
                        / (packet_loss.len() as f64),
                )
            })
            .collect();

        let packet_loss_len = packet_loss.len();
        if packet_loss_len < 1 {
            return None;
        }

        Some(packet_loss.into_iter().sum::<f64>() / packet_loss_len as f64)
    }

    /// Calculates and returns [`EstimatedConnectionQuality`] based on stats
    /// from this [`QualityMeter`].
    ///
    /// Returns `None` if some of the stats are empty.
    pub fn calculate(&mut self) -> Option<EstimatedConnectionQuality> {
        self.burn_stats();

        let latency = self.average_latency()?;
        let jitter = self.jitter()? as f64;
        let packet_loss = self.average_packet_loss()?;

        let effective_latency =
            jitter * Self::JITTER_FACTOR + latency + Self::CODEC_DELAY;

        // Calculate the R-Value (Transmission Rating Factor R) based on
        // Effective Latency. The voice quality drops more significantly
        // over 160ms so the R-Value is penalized more.
        let r = if effective_latency < 160.0 {
            Self::R0 - (effective_latency / 40.0)
        } else {
            Self::R0 - (effective_latency - 120.0) / 10.0
        };

        let r = r - (packet_loss * Self::P_LOSS_FACTOR);

        {
            use EstimatedConnectionQuality::{
                AllDissatisfied, ManyDissatisfied, Satisfied, SomeDissatisfied,
            };
            Some(if r < Self::R_LOWER_LIMIT_ALL_DISSATISFIED {
                AllDissatisfied
            } else if r < Self::R_LOWER_LIMIT_MANY_DISSATISFIED {
                ManyDissatisfied
            } else if r < Self::R_LOWER_LIMIT_SOME_DISSATISFIED {
                SomeDissatisfied
            } else {
                Satisfied
            })
        }
    }
}

/// Stat which can be burned when it will be outdated.
#[derive(Debug)]
pub struct BurningStat<T> {
    /// Timestamp of the RTC stat.
    timestamp: SystemTime,

    /// Actual stat.
    pub stat: T,
}

impl<T> BurningStat<T> {
    /// Returns new stat with a provided [`SystemTime`] as timestamp.
    pub fn new(stat: T, timestamp: SystemTime) -> Self {
        Self { timestamp, stat }
    }

    /// Returns `true` if this [`BurningStat`] is outdated.
    ///
    /// If this stat age more that 5 seconds, then this stat will be considered
    /// as outdated.
    pub fn should_be_burned(&self) -> bool {
        (self.timestamp + Duration::from_secs(5)) < SystemTime::now()
    }
}

/// Round trip time stat.
#[derive(Debug)]
pub struct Rtt(u64);

/// Jitter stat.
#[derive(Debug)]
pub struct Jitter(u64);

/// Packet loss percent stat.
#[derive(Debug)]
pub struct PacketLoss(u64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn very_good_call_quality() {
        let mut meter = QualityMeter::new();
        meter.add_packet_loss(
            SystemTime::now(),
            StatId("a".to_string()),
            0,
            1000,
        );
        meter.add_rtt(SystemTime::now(), 0);
        for jitter in &[0, 0, 0] {
            meter.add_jitter(
                SystemTime::now(),
                StatId("a".to_string()),
                *jitter,
            );
        }

        assert_eq!(
            meter.calculate().unwrap(),
            EstimatedConnectionQuality::Satisfied
        );
    }

    #[test]
    fn regular_normal_call() {
        let mut meter = QualityMeter::new();

        for jitter in &[0, 0, 0, 1, 3, 3, 0, 5, 0, 6, 0, 5, 0, 4, 2, 5, 2, 8] {
            meter.add_jitter(
                SystemTime::now(),
                StatId("a".to_string()),
                *jitter,
            );
        }
        for (packet_lost, packets_received) in &[
            (0, 45),
            (0, 50),
            (0, 95),
            (0, 96),
            (0, 146),
            (0, 158),
            (0, 197),
            (0, 217),
            (0, 248),
            (0, 279),
            (0, 279),
            (0, 299),
            (0, 336),
            (0, 349),
            (0, 396),
            (0, 401),
            (0, 452),
            (0, 457),
            (0, 503),
            (0, 528),
            (0, 600),
        ] {
            meter.add_packet_loss(
                SystemTime::now(),
                StatId("a".to_string()),
                *packet_lost,
                *packets_received,
            );
        }
        for rtt in &[1, 1, 2, 2, 2, 1, 1] {
            meter.add_rtt(SystemTime::now(), *rtt);
        }

        assert_eq!(
            meter.calculate().unwrap(),
            EstimatedConnectionQuality::Satisfied
        );
    }

    #[test]
    fn bad_call() {
        let mut meter = QualityMeter::new();

        for jitter in
            &[0, 3, 5, 1, 7, 40, 15, 1, 43, 5, 0, 30, 3, 4, 9, 40, 5, 8]
        {
            meter.add_jitter(
                SystemTime::now(),
                StatId("a".to_string()),
                *jitter,
            );
        }
        for (packet_lost, packets_received) in &[
            (0, 45),
            (1, 50),
            (3, 95),
            (5, 96),
            (8, 146),
            (8, 158),
            (8, 197),
            (10, 217),
            (12, 248),
            (12, 279),
            (13, 279),
            (13, 299),
            (20, 336),
            (30, 349),
            (43, 396),
            (55, 401),
            (76, 452),
            (80, 457),
            (80, 503),
            (81, 528),
            (81, 600),
        ] {
            meter.add_packet_loss(
                SystemTime::now(),
                StatId("a".to_string()),
                *packet_lost,
                *packets_received,
            );
        }
        for rtt in &[5, 3, 2, 1, 6, 5, 3] {
            meter.add_rtt(SystemTime::now(), *rtt);
        }

        assert_eq!(
            meter.calculate().unwrap(),
            EstimatedConnectionQuality::AllDissatisfied
        );
    }

    #[test]
    fn extremely_bad_call() {
        let mut meter = QualityMeter::new();
        meter.add_packet_loss(
            SystemTime::now(),
            StatId("a".to_string()),
            100,
            100,
        );
        meter.add_rtt(SystemTime::now(), 1000);
        for jitter in &[10, 1000, 3000] {
            meter.add_jitter(
                SystemTime::now(),
                StatId("a".to_string()),
                *jitter,
            );
        }

        assert_eq!(
            meter.calculate().unwrap(),
            EstimatedConnectionQuality::AllDissatisfied
        );
    }

    #[test]
    fn outdated_stats_are_burned() {
        let mut meter = QualityMeter::new();

        meter.add_jitter(
            SystemTime::now() - Duration::from_secs(5),
            StatId("a".to_string()),
            0,
        );
        meter.add_rtt(SystemTime::now() - Duration::from_secs(5), 0);
        meter.add_packet_loss(
            SystemTime::now() - Duration::from_secs(5),
            StatId("a".to_string()),
            0,
            0,
        );
        meter.add_jitter(SystemTime::now(), StatId("a".to_string()), 1);
        meter.add_rtt(SystemTime::now(), 1);
        meter.add_packet_loss(SystemTime::now(), StatId("a".to_string()), 1, 2);

        meter.calculate();

        let mut jitter = meter.jitter.remove(&StatId("a".to_string())).unwrap();
        assert_eq!(jitter.len(), 1);
        assert_eq!(jitter.pop().unwrap().stat.0, 1);

        assert_eq!(meter.rtt.len(), 1);
        assert_eq!(meter.rtt.pop().unwrap().stat.0, 1);

        let mut packet_loss =
            meter.packet_loss.remove(&StatId("a".to_string())).unwrap();
        assert_eq!(packet_loss.len(), 1);
        assert_eq!(packet_loss.pop().unwrap().stat.0, 33);
    }

    #[test]
    fn avg_connection_quality() {
        use EstimatedConnectionQuality::{
            AllDissatisfied, ManyDissatisfied, Satisfied, SomeDissatisfied,
        };

        for (first, second, result) in &[
            (Satisfied, Satisfied, Satisfied),
            (AllDissatisfied, AllDissatisfied, AllDissatisfied),
            (AllDissatisfied, Satisfied, ManyDissatisfied),
        ] {
            assert_eq!(
                EstimatedConnectionQuality::avg(*first, *second),
                *result,
                "{} avg {}",
                first,
                second
            );
        }
    }
}

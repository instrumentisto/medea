use std::{
    collections::HashMap,
    time::{Duration, SystemTime},
};

use derive_more::Display;
use medea_client_api_proto::stats::StatId;

fn burn<T>(stats: &mut Vec<BurningStat<T>>) {
    let burned_stats = std::mem::replace(stats, Vec::new());

    *stats = burned_stats
        .into_iter()
        .filter(|s| !s.should_be_burned())
        .collect();
}

#[derive(Clone, Debug, Copy, PartialEq, Eq, Display)]
pub enum EstimatedConnectionQuality {
    AllDissatisfied = 1,
    ManyDissatisfied = 2,
    SomeDissatisfied = 3,
    Satisfied = 4,
}

#[derive(Debug)]
pub struct QualityMeter {
    rtt: Vec<BurningStat<Rtt>>,
    jitter: HashMap<StatId, Vec<BurningStat<Jitter>>>,
    packet_loss: HashMap<StatId, Vec<BurningStat<PacketLoss>>>,
    last_packets_lost: HashMap<StatId, u64>,
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

    pub fn new() -> Self {
        Self {
            rtt: Vec::new(),
            jitter: HashMap::new(),
            packet_loss: HashMap::new(),
            last_packets_lost: HashMap::new(),
            last_total_packets: HashMap::new(),
        }
    }

    pub fn add_rtt(&mut self, timestamp: SystemTime, rtt: u64) {
        self.rtt.push(BurningStat::new(Rtt(rtt), timestamp));
    }

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

    pub fn add_packet_loss(
        &mut self,
        timestamp: SystemTime,
        stat_id: StatId,
        packets_lost: u64,
        total_packets: u64,
    ) {
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
            if *last_total_packets > packets_lost {
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
        if packet_loss > 1.0 {
            println!(
                "\n\n\n\n\n\n\nPACKET LOSS: {}; packets_lost: {}; \
                 total_packets: {}",
                packet_loss, packet_loss_at_period, total_packets_at_period
            );
        }

        self.packet_loss
            .entry(stat_id)
            .or_default()
            .push(BurningStat::new(
                PacketLoss((packet_loss * 100.0) as u64),
                timestamp,
            ));
    }

    fn burn_stats(&mut self) {
        burn(&mut self.rtt);
        self.jitter.values_mut().for_each(burn);
        self.packet_loss.values_mut().for_each(burn);
    }

    fn average_latency(&self) -> Option<f64> {
        if self.rtt.is_empty() {
            return None;
        }

        Some(
            self.rtt.iter().map(|s| s.stat.0 as f64).sum::<f64>()
                / (self.rtt.len() as f64),
        )
    }

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

    pub fn calculate(&mut self) -> Option<EstimatedConnectionQuality> {
        self.burn_stats();

        let latency = self.average_latency()?;
        let jitter = self.jitter()? as f64;
        let packet_loss = self.average_packet_loss()?;

        debug_assert!(latency >= 0.0, "latency cannot be negative");
        debug_assert!(jitter >= 0.0, "jitter cannot be negative");
        // TODO: panics
        debug_assert!(
            packet_loss >= 0.0 && packet_loss <= 100.0,
            "packet_loss must be between 0 and 100 but was {}",
            packet_loss
        );

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

        crate::log::prelude::info!("R = {}", r);

        {
            use EstimatedConnectionQuality::*;
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

#[derive(Debug)]
pub struct BurningStat<T> {
    timestamp: SystemTime,
    pub stat: T,
}

impl<T> BurningStat<T> {
    pub fn new(stat: T, timestamp: SystemTime) -> Self {
        Self { timestamp, stat }
    }

    pub fn should_be_burned(&self) -> bool {
        (self.timestamp + Duration::from_secs(5)) < SystemTime::now()
    }
}

#[derive(Debug)]
pub struct Rtt(u64);

#[derive(Debug)]
pub struct Jitter(u64);

#[derive(Debug)]
pub struct PacketLoss(u64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn very_good_call_quality() {
        let mut meter = QualityMeter::new();
        meter.add_packet_loss(0, 1000);
        meter.add_rtt(0);
        for jitter in &[0, 0, 0] {
            meter.add_jitter(*jitter);
        }

        assert_eq!(meter.calculate(), 4);
    }

    #[test]
    fn regular_normal_call() {
        let mut meter = QualityMeter::new();

        for jitter in &[0, 0, 0, 1, 3, 3, 0, 5, 0, 6, 0, 5, 0, 4, 2, 5, 2, 8] {
            meter.add_jitter(*jitter);
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
            meter.add_packet_loss(*packet_lost, *packets_received);
        }
        for rtt in &[1, 1, 2, 2, 2, 1, 1] {
            meter.add_rtt(*rtt);
        }

        assert_eq!(meter.calculate(), 4);
    }

    #[test]
    fn bad_call() {
        let mut meter = QualityMeter::new();

        for jitter in
            &[0, 3, 5, 1, 7, 40, 15, 1, 43, 5, 0, 30, 3, 4, 9, 40, 5, 8]
        {
            meter.add_jitter(*jitter);
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
            meter.add_packet_loss(*packet_lost, *packets_received);
        }
        for rtt in &[5, 3, 2, 1, 6, 5, 3] {
            meter.add_rtt(*rtt);
        }

        let quality_score = meter.calculate();
        assert_eq!(quality_score, 1);
    }

    #[test]
    fn extremely_bad_call() {
        let mut meter = QualityMeter::new();
        meter.add_packet_loss(100, 100);
        meter.add_rtt(1000);
        for jitter in &[10, 1000, 3000] {
            meter.add_jitter(*jitter);
        }

        assert_eq!(meter.calculate(), 1);
    }

    #[test]
    fn outdated_stats_are_burned() {
        let mut meter = QualityMeter::new();

        meter.jitter.push(BurningStat::new(
            Jitter(0),
            Instant::now() - Duration::from_secs(5),
        ));
        meter.rtt.push(BurningStat::new(
            Rtt(0),
            Instant::now() - Duration::from_secs(5),
        ));
        meter.packet_loss.push(BurningStat::new(
            PacketLoss(0),
            Instant::now() - Duration::from_secs(5),
        ));
        meter.add_jitter(1);
        meter.add_rtt(1);
        meter.add_packet_loss(1, 2);

        meter.calculate();

        assert_eq!(meter.jitter.len(), 1);
        assert_eq!(meter.jitter.pop().unwrap().stat.0, 1);

        assert_eq!(meter.rtt.len(), 1);
        assert_eq!(meter.rtt.pop().unwrap().stat.0, 1);

        assert_eq!(meter.packet_loss.len(), 1);
        assert_eq!(meter.packet_loss.pop().unwrap().stat.0, 50);
    }
}

use std::time::{Duration, Instant};

fn burn<T>(stats: &mut Vec<BurningStat<T>>) {
    let burned_stats = std::mem::replace(stats, Vec::new());

    *stats = burned_stats
        .into_iter()
        .filter(|s| !s.should_be_burned())
        .collect();
}

#[derive(Debug)]
pub struct QualityMeter {
    rtt: Vec<BurningStat<Rtt>>,
    jitter: Vec<BurningStat<Jitter>>,
    packet_loss: Vec<BurningStat<PacketLoss>>,
    last_packets_lost: u64,
    last_total_packets: u64,
}

impl QualityMeter {
    pub fn new() -> Self {
        Self {
            rtt: Vec::new(),
            jitter: Vec::new(),
            packet_loss: Vec::new(),
            last_packets_lost: 0,
            last_total_packets: 0,
        }
    }

    pub fn add_rtt(&mut self, rtt: u64) {
        self.rtt.push(BurningStat::new(Rtt(rtt), Instant::now()));
    }

    pub fn add_jitter(&mut self, jitter: u64) {
        self.jitter
            .push(BurningStat::new(Jitter(jitter), Instant::now()));
    }

    pub fn add_packet_loss(&mut self, packets_lost: u64, total_packets: u64) {
        if self.last_packets_lost > packets_lost {
            return;
        }
        if self.last_total_packets > total_packets {
            return;
        }

        let packet_loss_at_period = packets_lost - self.last_packets_lost;
        let total_packets_at_period = total_packets - self.last_total_packets;
        self.last_total_packets = total_packets;
        self.last_packets_lost = packets_lost;
        if total_packets_at_period == 0 {
            return;
        }

        let packet_loss =
            (packet_loss_at_period as f64) / (total_packets_at_period as f64);

        self.packet_loss.push(BurningStat::new(
            PacketLoss((packet_loss * 100.0) as u64),
            Instant::now(),
        ));
    }

    fn burn_stats(&mut self) {
        burn(&mut self.rtt);
        burn(&mut self.jitter);
        burn(&mut self.packet_loss);
    }

    fn average_latency(&self) -> f64 {
        if self.rtt.is_empty() {
            return 0.0;
        }

        self.rtt.iter().map(|s| s.stat.0 as f64).sum::<f64>()
            / (self.rtt.len() as f64)
    }

    fn jitter(&self) -> u64 {
        let mut jitter_iter = self.jitter.iter();
        let mut prev = jitter_iter.next().map(|s| s.stat.0).unwrap_or(0);
        jitter_iter.map(|j| j.stat.0).fold(0, |acc, val| {
            let out = if prev > val {
                acc + prev - val
            } else {
                acc + val - prev
            };
            prev = val;

            out
        })
    }

    fn average_packet_loss(&self) -> f64 {
        if self.packet_loss.is_empty() {
            return 0.0;
        }

        self.packet_loss
            .iter()
            .map(|s| s.stat.0 as f64)
            .sum::<f64>()
            / (self.packet_loss.len() as f64)
    }

    pub fn calculate(&mut self) -> f64 {
        self.burn_stats();

        let effective_latency =
            self.average_latency() + (self.jitter() as f64) * 2.0;

        let r = if effective_latency < 160.0 {
            100.0 - (effective_latency / 40.0)
        } else {
            100.0 - (effective_latency - 120.0) / 10.0
        };

        let r = r - (self.average_packet_loss() * 0.5);
        if r < 0.0 {
            return 1.0;
        } else if r > 100.0 {
            return 4.0;
        }

        1.0 + (0.030) * r + (0.000007) * r * (r - 60.0) * (100.0 - r)
    }
}

#[derive(Debug)]
pub struct BurningStat<T> {
    timestamp: Instant,
    pub stat: T,
}

impl<T> BurningStat<T> {
    pub fn new(stat: T, timestamp: Instant) -> Self {
        Self { timestamp, stat }
    }

    pub fn should_be_burned(&self) -> bool {
        (self.timestamp + Duration::from_secs(5)) < Instant::now()
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

        assert_eq!(meter.calculate(), 4.0);
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

        assert!(meter.calculate() > 3.9);
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
        assert!(quality_score > 1.3 && quality_score < 1.6);
    }

    #[test]
    fn extremely_bad_call() {
        let mut meter = QualityMeter::new();
        meter.add_packet_loss(100, 100);
        meter.add_rtt(1000);
        for jitter in &[10, 1000, 3000] {
            meter.add_jitter(*jitter);
        }

        assert_eq!(meter.calculate(), 1.0);
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

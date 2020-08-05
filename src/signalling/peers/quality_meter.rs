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

        let packet_loss = packet_loss_at_period / total_packets_at_period;

        self.packet_loss
            .push(BurningStat::new(PacketLoss(packet_loss), Instant::now()));
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

        let foo = self
            .packet_loss
            .iter()
            .map(|s| s.stat.0 as f64)
            .sum::<f64>();
        let bar = (self.packet_loss.len() as f64);

        foo / bar
    }

    pub fn calculate(&mut self) -> f64 {
        self.burn_stats();

        let effective_latency =
            (self.average_latency() + (self.jitter() as f64) * 2.0);

        let r = if effective_latency < 160.0 {
            100.0 - (effective_latency / 40.0)
        } else {
            100.0 - (effective_latency - 120.0) / 10.0
        };

        let r = r - (self.average_packet_loss() * 2.5);

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
        (self.timestamp + Duration::from_secs(30)) < Instant::now()
    }
}

#[derive(Debug)]
pub struct Rtt(u64);

#[derive(Debug)]
pub struct Jitter(u64);

#[derive(Debug)]
pub struct PacketLoss(u64);

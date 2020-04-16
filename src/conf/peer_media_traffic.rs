//! `Peer` media traffic watcher configuration.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct PeerMediaTraffic {
    /// Duration after which media server will consider `Peer`'s media traffic
    /// stats as invalid and will remove this `Peer`.
    #[default(Duration::from_secs(10))]
    #[serde(with = "humantime_serde")]
    pub peer_validity_timeout: Duration,

    /// Duration after which media server will consider that `Peer`'s media
    /// traffic is stopped.
    #[default(Duration::from_secs(10))]
    #[serde(with = "humantime_serde")]
    pub traffic_flowing_timeout: Duration,

    /// Duration within which media server should receive signal of `Peer`
    /// start from all sources.
    ///
    /// If media server wouldn't receive those signals, then this `Peer` will
    /// be removed within this duration.
    #[default(Duration::from_secs(15))]
    #[serde(with = "humantime_serde")]
    pub peer_init_timeout: Duration,
}

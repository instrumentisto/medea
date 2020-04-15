use std::time::Duration;

use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct MediaTraffic {
    #[default(Duration::from_secs(10))]
    #[serde(with = "humantime_serde")]
    pub peer_validity_timeout: Duration,

    #[default(Duration::from_secs(10))]
    #[serde(with = "humantime_serde")]
    pub traffic_flowing_timeout: Duration,

    #[default(Duration::from_secs(15))]
    #[serde(with = "humantime_serde")]
    pub peer_init_timeout: Duration,
}

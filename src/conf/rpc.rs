use serde::{Deserialize, Serialize};
use smart_default::*;

use std::time::Duration;

/// RPC connection settings.
#[derive(Clone, Debug, Serialize, Deserialize, SmartDefault)]
pub struct Rpc {
    /// Duration, after which remote RPC client will be considered idle if no
    /// heartbeat messages received.
    #[serde(with = "serde_humantime")]
    #[default(Duration::from_secs(10))]
    pub idle_timeout: Duration,
}

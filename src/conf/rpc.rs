//! RPC connection settings.
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

use std::time::Duration;

/// RPC connection settings.
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct Rpc {
    /// Duration, after which remote RPC client will be considered idle if no
    /// heartbeat messages received. Defaults to `10s`.
    #[default(Duration::from_secs(10))]
    #[serde(with = "humantime_serde")]
    pub idle_timeout: Duration,

    /// Duration, after which the server deletes the client session if
    /// the remote RPC client does not reconnect after it is idle.
    #[default(Duration::from_secs(10))]
    #[serde(with = "humantime_serde")]
    pub reconnect_timeout: Duration,
}

use serde::{Deserialize, Serialize};
use smart_default::*;

use std::time::Duration;

/// Server represents [`Server`] configuration section.
#[derive(Clone, Debug, Serialize, Deserialize, SmartDefault)]
pub struct Rpc {
    /// Timeout for [`WsSession`] to wait ping message from [`Web Client`].
    #[serde(deserialize_with = "serde_humantime::deserialize")]
    #[serde(serialize_with = "crate::utils::duration::serialize")]
    #[default(Duration::from_secs(10))]
    pub idle_timeout: Duration,
}

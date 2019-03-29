use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::conf::duration;

/// Server represents [`Server`] configuration section.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Rpc {
    /// Timeout for [`WsSession`] to wait ping message from [`Web Client`].
    #[serde(serialize_with = "duration::serialize")]
    #[serde(deserialize_with = "duration::deserialize")]
    pub idle_timeout: Duration,
}

/// Default returns default configuration parameters of [`Server`] section.
impl Default for Rpc {
    fn default() -> Self {
        Self {
            idle_timeout: Duration::from_secs(10),
        }
    }
}

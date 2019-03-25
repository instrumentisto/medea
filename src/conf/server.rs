use std::time::Duration;

use serde_derive::{Deserialize, Serialize};

use crate::conf::duration;

/// Server represents [`Server`] configuration section.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Server {
    /// Timeout for [`WsSession`] to wait ping message from [`Web Client`].
    #[serde(serialize_with = "duration::serialize")]
    #[serde(deserialize_with = "duration::deserialize")]
    pub client_idle_timeout: Duration,
}

/// Default returns default configuration parameters of [`Server`] section.
impl Default for Server {
    fn default() -> Self {
        Self {
            client_idle_timeout: Duration::from_secs(10),
        }
    }
}

use std::time::Duration;

use serde_derive::{Deserialize, Serialize};

use crate::settings::duration;

/// Server represents [server] configuration section.
#[derive(Debug, Deserialize, Serialize)]
pub struct Server {
    /// Timeout for websocket session to wait message from [`Web Client`].
    #[serde(serialize_with = "duration::serialize")]
    #[serde(deserialize_with = "duration::deserialize")]
    client_idle_timeout: Duration,
}

/// Default returns default configuration parameters of [server] section.
impl Default for Server {
    fn default() -> Server {
        Server {
            client_idle_timeout: Duration::from_secs(10),
        }
    }
}

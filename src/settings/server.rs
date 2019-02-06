use std::time::Duration;

use serde_derive::{Deserialize, Serialize};

use crate::settings::duration;

#[derive(Debug, Deserialize, Serialize)]
pub struct Server {
    #[serde(serialize_with = "duration::serialize")]
    #[serde(deserialize_with = "duration::deserialize")]
    client_idle_timeout: Duration,
}

impl Default for Server {
    fn default() -> Server {
        Server {
            client_idle_timeout: Duration::from_secs(10),
        }
    }
}

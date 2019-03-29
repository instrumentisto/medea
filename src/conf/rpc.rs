use serde::{Deserialize, Serialize};
use smart_default::*;

use std::time::Duration;

use crate::conf::duration;

/// Server represents [`Server`] configuration section.
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
pub struct Rpc {
    /// Timeout for [`WsSession`] to wait ping message from [`Web Client`].
    #[serde(serialize_with = "duration::serialize")]
    #[serde(deserialize_with = "duration::deserialize")]
    #[default(Duration::from_secs(10))]
    pub idle_timeout: Duration,
}

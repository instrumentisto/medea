//! Application shutdown settings.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

/// Application shutdown settings.
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct Shutdown {
    /// Maximum duration given to shutdown the whole application gracefully.
    #[default(Duration::from_secs(5))]
    #[serde(with = "humantime_serde")]
    pub timeout: Duration,
}

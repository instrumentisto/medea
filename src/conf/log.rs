//! Logging settings.

use std::str::FromStr as _;

use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

/// Logging settings.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, SmartDefault)]
#[serde(default)]
pub struct Log {
    /// Maximum allowed level of application log entries.
    /// Defaults to `INFO`.
    #[default(String::from("INFO"))]
    pub level: String,
}

impl Log {
    /// Returns configured application logging level. `None` if disabled.
    pub fn level(&self) -> Option<slog::Level> {
        slog::Level::from_str(&self.level).ok()
    }
}

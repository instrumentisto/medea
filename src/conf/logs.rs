//! Logs settings.
use std::str::FromStr as _;

use serde::{Deserialize, Serialize};
use smart_default::*;

/// Logs settings.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct Logs {
    /// Application log settings.
    pub app: AppLog,
}

/// Application log settings.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, SmartDefault)]
#[serde(default)]
pub struct AppLog {
    // Maximum allowed level of application log entries.
    //
    // Defaults to `INFO`.
    #[default(String::from("INFO"))]
    level: String,
}

impl AppLog {
    /// Returns configured application logging level. None if disabled.
    pub fn level(&self) -> Option<slog::Level> {
        slog::Level::from_str(&self.level).ok()
    }
}

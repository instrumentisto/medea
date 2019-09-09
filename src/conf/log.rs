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

#[cfg(test)]
mod log_conf_specs {
    use std::env;

    use serial_test_derive::serial;

    use crate::conf::Conf;

    #[test]
    #[serial]
    fn overrides_defaults() {
        let default_conf = Conf::default();

        env::set_var("MEDEA_LOG.LEVEL", "DEBUG");
        let env_conf = Conf::parse().unwrap();
        env::remove_var("MEDEA_LOG.LEVEL");

        assert_ne!(default_conf.log.level, env_conf.log.level);
    }
}

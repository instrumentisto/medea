//! Logging settings.

use std::{borrow::Cow, str::FromStr as _};

use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

/// Logging settings.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, SmartDefault)]
#[serde(default)]
pub struct Log {
    /// Maximum allowed level of application log entries.
    /// Defaults to `INFO`.
    #[default("INFO")]
    pub level: Cow<'static, str>,
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

        env::set_var("MEDEA_LOG__LEVEL", "WARN");
        let env_conf = Conf::parse().unwrap();
        env::set_var("MEDEA_LOG__LEVEL", "OFF");

        assert_ne!(default_conf.log.level(), env_conf.log.level());
        assert_eq!(env_conf.log.level(), Some(slog::Level::Warning));
        assert_eq!(Conf::parse().unwrap().log.level(), None);
    }
}

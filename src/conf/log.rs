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
    #[default = "INFO"]
    pub level: Cow<'static, str>,
}

impl Log {
    /// Returns configured application logging level, or [`None`] if disabled.
    #[inline]
    #[must_use]
    pub fn level(&self) -> Option<slog::Level> {
        slog::Level::from_str(&self.level).ok()
    }
}

#[cfg(test)]
mod log_conf_specs {
    use serial_test::serial;

    use crate::{conf::Conf, overrided_by_env_conf};

    #[test]
    #[serial]
    fn overrides_defaults() {
        let default_conf = Conf::default();

        let env_conf = overrided_by_env_conf!(
            "MEDEA_LOG__LEVEL" => "WARN",
        );
        assert_ne!(default_conf.log.level(), env_conf.log.level());
        assert_eq!(env_conf.log.level(), Some(slog::Level::Warning));

        let none_lvl = overrided_by_env_conf!(
            "MEDEA_LOG__LEVEL" => "OFF",
        );
        assert_eq!(none_lvl.log.level(), None);
    }
}

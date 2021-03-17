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

#[cfg(test)]
mod spec {
    use std::time::Duration;

    use serial_test::serial;

    use crate::{conf::Conf, overrided_by_env_conf};

    #[test]
    #[serial]
    fn overrides_defaults() {
        let default_conf = Conf::default();
        let env_conf = overrided_by_env_conf!(
            "MEDEA_SHUTDOWN__TIMEOUT" => "20s",
        );

        assert_ne!(default_conf.shutdown.timeout, env_conf.shutdown.timeout);
        assert_eq!(env_conf.shutdown.timeout, Duration::from_secs(20));
    }
}

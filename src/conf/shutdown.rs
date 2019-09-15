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
mod shutdown_conf_specs {
    use std::env;

    use serial_test_derive::serial;

    use crate::conf::Conf;

    #[test]
    #[serial]
    fn overrides_defaults() {
        let default_conf = Conf::default();

        env::set_var("MEDEA_SHUTDOWN__TIMEOUT", "20s");
        let env_conf = Conf::parse().unwrap();
        env::remove_var("MEDEA_SHUTDOWN__TIMEOUT");

        assert_ne!(default_conf.shutdown.timeout, env_conf.shutdown.timeout);
    }
}

//! RPC connection settings.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

/// RPC connection settings.
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct Rpc {
    /// Duration, after which remote RPC client will be considered idle if no
    /// heartbeat messages received. Defaults to `10s`.
    #[default(Duration::from_secs(10))]
    #[serde(with = "humantime_serde")]
    pub idle_timeout: Duration,

    /// Duration, after which the server deletes the client session if
    /// the remote RPC client does not reconnect after it is idle.
    #[default(Duration::from_secs(10))]
    #[serde(with = "humantime_serde")]
    pub reconnect_timeout: Duration,
}

// TODO: are you sure its not failing?
#[cfg(test)]
mod log_conf_specs {
    use std::env;

    use serial_test_derive::serial;

    use crate::conf::Conf;

    #[test]
    #[serial]
    fn overrides_defaults() {
        let default_conf = Conf::default();

        env::set_var("MEDEA_RPC.IDLE_TIMEOUT", "20s");
        env::set_var("MEDEA_RPC.RECONNECT_TIMEOUT", "30s");
        let env_conf = Conf::parse().unwrap();
        env::remove_var("MEDEA_RPC.IDLE_TIMEOUT");
        env::remove_var("MEDEA_RPC.RECONNECT_TIMEOUT");

        assert_ne!(default_conf.rpc.idle_timeout, env_conf.rpc.idle_timeout);
        assert_ne!(
            default_conf.rpc.reconnect_timeout,
            env_conf.rpc.reconnect_timeout
        );
    }
}

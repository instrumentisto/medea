//! RPC connection settings.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

/// RPC connection settings.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct Rpc {
    /// Default duration, after which remote RPC client will be considered IDLE
    /// if no heartbeat messages received.
    ///
    /// This duration can be overwrited for some `Member` by Control API spec.
    ///
    /// Defaults to `10s`.
    #[default(Duration::from_secs(10))]
    #[serde(with = "humantime_serde")]
    pub default_idle_timeout: Duration,

    /// Default duration, after which the server deletes the client session if
    /// the remote RPC client does not reconnect after it is IDLE.
    ///
    /// This duration can be overwrited for some `Member` by Control API spec.
    ///
    /// Defaults to `10s`.
    #[default(Duration::from_secs(10))]
    #[serde(with = "humantime_serde")]
    pub default_reconnect_timeout: Duration,

    /// Default interval of sending `Ping`s from the server to the client.
    ///
    /// This duration can be overwrited for some `Member` by Control API spec.
    ///
    /// Defaults to `3s`.
    #[default(Duration::from_secs(3))]
    #[serde(with = "humantime_serde")]
    pub default_ping_interval: Duration,
}

#[cfg(test)]
mod spec {
    use std::{fs, time::Duration};

    use serial_test_derive::serial;

    use crate::{
        conf::{Conf, APP_CONF_PATH_ENV_VAR_NAME},
        overrided_by_env_conf,
    };

    #[test]
    #[serial]
    fn overrides_defaults() {
        let default_conf = Conf::default();
        let env_conf = overrided_by_env_conf!(
            "MEDEA_RPC__DEFAULT_IDLE_TIMEOUT" => "20s",
            "MEDEA_RPC__DEFAULT_RECONNECT_TIMEOUT" => "30s",
        );

        assert_ne!(
            default_conf.rpc.default_idle_timeout,
            env_conf.rpc.default_idle_timeout
        );
        assert_ne!(
            default_conf.rpc.default_reconnect_timeout,
            env_conf.rpc.default_reconnect_timeout
        );

        assert_eq!(env_conf.rpc.default_idle_timeout, Duration::from_secs(20));
        assert_eq!(
            env_conf.rpc.default_reconnect_timeout,
            Duration::from_secs(30)
        );
    }

    #[test]
    #[serial]
    fn conf_parse_spec_file_overrides_defaults() {
        // Don't delete me! Otherwise temporary dir will be deleted.
        let dir = tempfile::tempdir().unwrap();
        let conf_path =
            dir.path().join("test_config.toml").display().to_string();

        let data = "[rpc]\ndefault_idle_timeout = \"45s\"".to_owned();
        fs::write(&conf_path, data).unwrap();

        let new_config = overrided_by_env_conf!(
            APP_CONF_PATH_ENV_VAR_NAME => &conf_path,
        );

        assert_eq!(
            new_config.rpc.default_idle_timeout,
            Duration::from_secs(45)
        );
        assert_ne!(
            new_config.rpc.default_idle_timeout,
            Conf::default().rpc.default_idle_timeout
        );
    }

    #[test]
    #[serial]
    fn conf_parse_spec_env_overrides_file() {
        // Don't delete me! Otherwise temporary dir will be deleted.
        let dir = tempfile::tempdir().unwrap();
        let conf_path =
            dir.path().join("test_config.toml").display().to_string();

        let data = "[rpc]\ndefault_idle_timeout = \"47s\"".to_owned();
        fs::write(&conf_path, data).unwrap();

        let file_config = overrided_by_env_conf!(
            APP_CONF_PATH_ENV_VAR_NAME => &conf_path,
        );
        let file_env_config = overrided_by_env_conf!(
            APP_CONF_PATH_ENV_VAR_NAME => &conf_path,
            "MEDEA_RPC__DEFAULT_IDLE_TIMEOUT" => "48s",
        );

        assert_eq!(
            file_config.rpc.default_idle_timeout,
            Duration::from_secs(47)
        );
        assert_eq!(
            file_env_config.rpc.default_idle_timeout,
            Duration::from_secs(48)
        );
    }
}

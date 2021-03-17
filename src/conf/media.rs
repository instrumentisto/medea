//! `Peer` media traffic watcher configuration.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct Media {
    /// Max duration of media-flow lag, after which `on_stop` callback will be
    /// fired.
    #[default(Duration::from_secs(15))]
    #[serde(with = "humantime_serde")]
    pub max_lag: Duration,

    /// Timeout for peer to become active after it was created.
    #[default(Duration::from_secs(15))]
    #[serde(with = "humantime_serde")]
    pub init_timeout: Duration,
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
            "MEDEA_MEDIA__MAX_LAG" => "502ms",
            "MEDEA_MEDIA__INIT_TIMEOUT" => "503ms",
        );

        assert_ne!(default_conf.media.max_lag, env_conf.media.max_lag);
        assert_eq!(env_conf.media.max_lag, Duration::from_millis(502));

        assert_ne!(
            default_conf.media.init_timeout,
            env_conf.media.init_timeout,
        );
        assert_eq!(env_conf.media.init_timeout, Duration::from_millis(503));
    }
}

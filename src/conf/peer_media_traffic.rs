//! `Peer` media traffic watcher configuration.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct PeerMediaTraffic {
    /// Duration after which media server will consider `Peer`'s media traffic
    /// stats as invalid and will remove this `Peer`.
    #[default(Duration::from_secs(10))]
    #[serde(with = "humantime_serde")]
    pub peer_validity_timeout: Duration,

    /// Duration after which media server will consider that `Peer`'s media
    /// traffic is stopped.
    #[default(Duration::from_secs(10))]
    #[serde(with = "humantime_serde")]
    pub traffic_flowing_timeout: Duration,

    /// Duration within which media server should receive signal of `Peer`
    /// start from all sources.
    ///
    /// If media server wouldn't receive those signals, then this `Peer` will
    /// be removed within this duration.
    #[default(Duration::from_secs(15))]
    #[serde(with = "humantime_serde")]
    pub peer_init_timeout: Duration,
}

#[cfg(test)]
mod spec {
    use std::time::Duration;

    use serial_test_derive::serial;

    use crate::{conf::Conf, overrided_by_env_conf};

    #[test]
    #[serial]
    fn overrides_defaults() {
        let default_conf = Conf::default();
        let env_conf = overrided_by_env_conf!(
            "MEDEA_PEER_MEDIA_TRAFFIC__PEER_VALIDITY_TIMEOUT" => "501ms",
            "MEDEA_PEER_MEDIA_TRAFFIC__TRAFFIC_FLOWING_TIMEOUT" => "502ms",
            "MEDEA_PEER_MEDIA_TRAFFIC__PEER_INIT_TIMEOUT" => "503ms",
        );

        assert_ne!(
            default_conf.peer_media_traffic.peer_validity_timeout,
            env_conf.peer_media_traffic.peer_validity_timeout,
        );
        assert_eq!(
            env_conf.peer_media_traffic.peer_validity_timeout,
            Duration::from_millis(501)
        );

        assert_ne!(
            default_conf.peer_media_traffic.traffic_flowing_timeout,
            env_conf.peer_media_traffic.traffic_flowing_timeout,
        );
        assert_eq!(
            env_conf.peer_media_traffic.traffic_flowing_timeout,
            Duration::from_millis(502)
        );

        assert_ne!(
            default_conf.peer_media_traffic.peer_init_timeout,
            env_conf.peer_media_traffic.peer_init_timeout,
        );
        assert_eq!(
            env_conf.peer_media_traffic.peer_init_timeout,
            Duration::from_millis(503)
        );
    }
}

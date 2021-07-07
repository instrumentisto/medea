//! STUN/TURN server settings.

use std::{borrow::Cow, time::Duration};

use deadpool::Runtime;
use redis::ConnectionInfo;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

/// [STUN]/[TURN] servers settings.
///
/// [TURN]: https://webrtcglossary.com/turn/
/// [STUN]: https://webrtcglossary.com/stun/
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(tag = "mode")]
pub enum Turn {
    /// Settings for the [Coturn] server.
    ///
    /// [Coturn]: https://github.com/coturn/coturn
    #[serde(rename = "coturn")]
    #[default]
    Coturn { coturn: Coturn },

    /// Static [TURN]/[STUN] servers list.
    ///
    /// [TURN]: https://webrtcglossary.com/turn/
    /// [STUN]: https://webrtcglossary.com/stun/
    #[serde(rename = "static")]
    Static { r#static: StaticServers },
}

/// [`RtcIceServer`]s list for `static` mode [`Turn`] configuration.
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
pub struct StaticServers {
    pub servers: Vec<RtcIceServer>,
}

/// Defines how to connect to the [TURN]/[STUN] server.
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
pub struct RtcIceServer {
    /// URLs which can be used to connect to the [TURN]/[STUN] server.
    ///
    /// [TURN]: https://webrtcglossary.com/turn/
    /// [STUN]: https://webrtcglossary.com/stun/
    pub urls: Vec<String>,

    /// Username to use during the authentication process.
    pub username: Option<String>,

    /// The credential to use when logging into the server.
    pub credential: Option<String>,
}

/// [Coturn] server settings for `coturn` mode [`Turn`] configuration.
///
/// [Coturn]: https://github.com/coturn/coturn
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct Coturn {
    /// Host of STUN/TURN server.
    ///
    /// Defaults to `localhost`.
    #[default = "localhost"]
    pub host: Cow<'static, str>,

    /// Port of TURN server.
    ///
    /// Defaults to `3478`.
    #[default = 3478]
    pub port: u16,

    /// Name of static user to authenticate on TURN server as.
    ///
    /// Defaults to `USER`.
    #[default = "USER"]
    pub user: Cow<'static, str>,

    /// Password of static user to authenticate on TURN server with.
    ///
    /// Defaults to `PASS`.
    #[default = "PASS"]
    pub pass: Cow<'static, str>,

    /// Database settings
    pub db: Db,

    /// Admin interface settings.
    pub cli: CoturnCli,
}

impl Coturn {
    /// Builds [`String`] addr from `host` and `port`.
    #[inline]
    #[must_use]
    pub fn addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct Db {
    /// [Redis] database settings.
    ///
    /// [Redis]: https://redis.io
    pub redis: Redis,
}

/// Setting of [Redis] database server which backs [Coturn] storage.
///
/// [Coturn]: https://github.com/coturn/coturn
/// [Redis]: https://redis.io
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct Redis {
    /// Host of Redis database server.
    ///
    /// Defaults to `127.0.0.1`.
    #[default = "127.0.0.1"]
    pub host: Cow<'static, str>,

    /// Port of Redis database server for client connections.
    ///
    /// Defaults to `6379`.
    #[default = 6379]
    pub port: u16,

    /// User to authenticate on Redis database server as.
    ///
    /// Defaults to empty value.
    #[default = ""]
    pub user: Cow<'static, str>,

    /// Password to authenticate on Redis database server with.
    ///
    /// Defaults to `turn`.
    #[default = "turn"]
    pub pass: Cow<'static, str>,

    /// The Redis database number to use. This is usually `0`.
    ///
    /// Defaults to `0`.
    #[default = 0]
    pub db_number: i64,

    // TODO: replace with PoolConfig
    /// Timeout for establishing connection with Redis database server.
    #[default(Duration::from_secs(5))]
    #[serde(with = "humantime_serde")]
    pub connect_timeout: Duration,
}

impl From<&Redis> for ConnectionInfo {
    fn from(cf: &Redis) -> Self {
        Self {
            username: Some(cf.user.to_string()).filter(|u| !u.is_empty()),
            addr: Box::new(redis::ConnectionAddr::Tcp(
                cf.host.to_string(),
                cf.port,
            )),
            db: cf.db_number,
            passwd: Some(cf.pass.to_string()).filter(|p| !p.is_empty()),
        }
    }
}

/// Settings of [Coturn]'s admin interface.
///
/// [Coturn]: https://github.com/coturn/coturn
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct CoturnCli {
    /// Host of admin interface.
    ///
    /// Defaults to `127.0.0.1`.
    #[default = "127.0.0.1"]
    pub host: Cow<'static, str>,

    /// Port of interface for [Telnet] connections.
    ///
    /// Defaults to `5766`.
    ///
    /// [Telnet]: https://en.wikipedia.org/wiki/Telnet
    #[default = 5766]
    pub port: u16,

    /// Password to authenticate on admin interface with.
    ///
    /// Defaults to `turn`.
    #[default = "turn"]
    pub pass: Cow<'static, str>,

    /// Settings for pool of connections with admin interface.
    pub pool: PoolConfig,
}

/// Settings for pool of connections with [Coturn]'s admin interface.
///
/// [Coturn]: https://github.com/coturn/coturn
#[derive(Clone, Copy, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct PoolConfig {
    /// Maximum size of the pool.
    ///
    /// Defaults to `16`.
    #[default = 16]
    pub max_size: usize,

    /// Waiting timeout for an available connection in the pool.
    ///
    /// Defaults to `2s`.
    #[default(Duration::from_secs(2))]
    #[serde(with = "humantime_serde")]
    pub wait_timeout: Duration,

    /// Timeout for establishing connection.
    ///
    /// Defaults to `2s`.
    #[default(Duration::from_secs(2))]
    #[serde(with = "humantime_serde")]
    pub connect_timeout: Duration,

    /// Timeout for recycling established connection.
    ///
    /// Defaults to `2s`.
    #[default(Duration::from_secs(2))]
    #[serde(with = "humantime_serde")]
    pub recycle_timeout: Duration,
}

impl From<PoolConfig> for deadpool::managed::PoolConfig {
    fn from(cfg: PoolConfig) -> Self {
        Self {
            max_size: cfg.max_size,
            timeouts: deadpool::managed::Timeouts {
                wait: (cfg.wait_timeout.as_nanos() != 0)
                    .then(|| cfg.wait_timeout),
                create: (cfg.connect_timeout.as_nanos() != 0)
                    .then(|| cfg.connect_timeout),
                recycle: (cfg.recycle_timeout.as_nanos() != 0)
                    .then(|| cfg.recycle_timeout),
            },

            runtime: Runtime::Tokio1,
        }
    }
}

#[cfg(test)]
mod spec {
    use serial_test::serial;

    use crate::{conf::Conf, overrided_by_env_conf};

    use super::*;

    #[test]
    #[serial]
    fn redis_db_overrides_defaults() {
        let default_conf = Conf::default();
        let env_conf = overrided_by_env_conf!(
            "MEDEA_TURN__MODE" => "coturn",
            "MEDEA_TURN__COTURN__DB__REDIS__HOST" => "5.5.5.5",
            "MEDEA_TURN__COTURN__DB__REDIS__PORT" => "1234",
            "MEDEA_TURN__COTURN__DB__REDIS__PASS" => "hellofellow",
            "MEDEA_TURN__COTURN__DB__REDIS__DB_NUMBER" => "10",
            "MEDEA_TURN__COTURN__DB__REDIS__CONNECT_TIMEOUT" => "10s",
        );
        let default_coturn = if let Turn::Coturn { coturn } = default_conf.turn
        {
            coturn
        } else {
            unreachable!();
        };
        let env_coturn = if let Turn::Coturn { coturn } = env_conf.turn {
            coturn
        } else {
            unreachable!();
        };

        assert_ne!(default_coturn.db.redis.host, env_coturn.db.redis.host,);
        assert_ne!(default_coturn.db.redis.port, env_coturn.db.redis.port,);
        assert_ne!(default_coturn.db.redis.pass, env_coturn.db.redis.pass,);
        assert_ne!(
            default_coturn.db.redis.db_number,
            env_coturn.db.redis.db_number,
        );
        assert_ne!(
            default_coturn.db.redis.connect_timeout,
            env_coturn.db.redis.connect_timeout,
        );

        assert_eq!(env_coturn.db.redis.host, "5.5.5.5");
        assert_eq!(env_coturn.db.redis.port, 1234);
        assert_eq!(
            env_coturn.db.redis.connect_timeout,
            Duration::from_secs(10),
        );
    }

    #[test]
    #[serial]
    fn overrides_defaults() {
        let default_conf = Conf::default();
        let env_conf = overrided_by_env_conf!(
            "MEDEA_TURN__MODE" => "coturn",
            "MEDEA_TURN__COTURN__HOST" => "example.com",
            "MEDEA_TURN__COTURN__PORT" => "1234",
            "MEDEA_TURN__COTURN__USER" => "ferris",
            "MEDEA_TURN__COTURN__PASS" => "qwerty",
        );

        let default_coturn = if let Turn::Coturn { coturn } = default_conf.turn
        {
            coturn
        } else {
            unreachable!();
        };
        let env_coturn = if let Turn::Coturn { coturn } = env_conf.turn {
            coturn
        } else {
            unreachable!();
        };

        assert_ne!(default_coturn.host, env_coturn.host);
        assert_ne!(default_coturn.port, env_coturn.port);
        assert_ne!(default_coturn.user, env_coturn.user);
        assert_ne!(default_coturn.pass, env_coturn.pass);

        assert_eq!(default_coturn.host, "example.com");
        assert_eq!(default_coturn.port, 1234);
        assert_eq!(default_coturn.addr(), "example.com:1234");
    }

    #[test]
    #[serial]
    fn coturn_cli() {
        let default_conf = Conf::default();
        let env_conf = overrided_by_env_conf!(
            "MEDEA_TURN__MODE" => "coturn",
            "MEDEA_TURN__COTURN__CLI__HOST" => "4.4.4.4",
            "MEDEA_TURN__COTURN__CLI__PORT" => "1234",
            "MEDEA_TURN__COTURN__CLI__PASS" => "clipass",
        );

        let default_coturn = if let Turn::Coturn { coturn } = default_conf.turn
        {
            coturn
        } else {
            unreachable!();
        };
        let env_coturn = if let Turn::Coturn { coturn } = env_conf.turn {
            coturn
        } else {
            unreachable!();
        };

        assert_ne!(default_coturn.cli.host, env_coturn.cli.host);
        assert_ne!(default_coturn.cli.port, env_coturn.cli.port);
        assert_ne!(default_coturn.cli.pass, env_coturn.cli.pass);

        assert_eq!(env_coturn.cli.host, "4.4.4.4");
        assert_eq!(env_coturn.cli.port, 1234);
        assert_eq!(env_coturn.cli.pass, "clipass");
    }

    #[test]
    #[serial]
    fn coturn_cli_pool() {
        let default_conf = Conf::default();
        let env_conf = overrided_by_env_conf!(
            "MEDEA_TURN__MODE" => "coturn",
            "MEDEA_TURN__COTURN__CLI__POOL__MAX_SIZE" => "10",
            "MEDEA_TURN__COTURN__CLI__POOL__WAIT_TIMEOUT" => "1s",
            "MEDEA_TURN__COTURN__CLI__POOL__CONNECT_TIMEOUT" => "4s",
            "MEDEA_TURN__COTURN__CLI__POOL__RECYCLE_TIMEOUT" => "3s",
        );

        let default_coturn = if let Turn::Coturn { coturn } = default_conf.turn
        {
            coturn
        } else {
            unreachable!();
        };
        let env_coturn = if let Turn::Coturn { coturn } = env_conf.turn {
            coturn
        } else {
            unreachable!();
        };

        assert_ne!(
            default_coturn.cli.pool.max_size,
            env_coturn.cli.pool.max_size,
        );
        assert_ne!(
            default_coturn.cli.pool.wait_timeout,
            env_coturn.cli.pool.wait_timeout,
        );
        assert_ne!(
            default_coturn.cli.pool.connect_timeout,
            env_coturn.cli.pool.connect_timeout,
        );
        assert_ne!(
            default_coturn.cli.pool.recycle_timeout,
            env_coturn.cli.pool.recycle_timeout,
        );

        assert_eq!(env_coturn.cli.pool.max_size, 10);
        assert_eq!(env_coturn.cli.pool.wait_timeout, Duration::from_secs(1));
        assert_eq!(env_coturn.cli.pool.connect_timeout, Duration::from_secs(4));
        assert_eq!(env_coturn.cli.pool.recycle_timeout, Duration::from_secs(3));
    }

    #[test]
    fn into_deadpool_pool_config() {
        let pool_cfg = PoolConfig {
            max_size: 6,
            wait_timeout: Duration::default(),
            connect_timeout: Duration::from_secs(0),
            recycle_timeout: Duration::from_secs(3),
        };
        let pool_cfg: deadpool::managed::PoolConfig = pool_cfg.into();

        assert_eq!(pool_cfg.max_size, 6);
        assert!(pool_cfg.timeouts.wait.is_none());
        assert!(pool_cfg.timeouts.create.is_none());
        assert_eq!(pool_cfg.timeouts.recycle, Some(Duration::from_secs(3)));
    }
}

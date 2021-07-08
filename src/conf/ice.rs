//! [ICE] servers settings.
//!
//! [ICE]: https://webrtcglossary.com/ice

use std::{
    borrow::{Cow, ToOwned},
    collections::HashMap,
    time::Duration,
};

use deadpool::Runtime;
use redis::ConnectionInfo;
use serde::{de::Error as _, Deserialize, Deserializer, Serialize};
use smart_default::SmartDefault;

/// [ICE] servers settings.
///
/// [ICE]: https://webrtcglossary.com/ice
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct Ice {
    /// Default [`Kind`] of [ICE] servers to be used for [WebRTC].
    ///
    /// Defaults to [`Kind::Coturn`].
    ///
    /// [ICE]: https://webrtcglossary.com/ice
    /// [WebRTC]: https://webrtcglossary.com/webrtc
    pub default: Kind,

    /// Managed [Coturn] server settings.
    ///
    /// [Coturn]: https://github.com/coturn/coturn
    pub coturn: Coturn,

    /// List of static unmanaged [STUN]/[TURN] servers.
    ///
    /// [STUN]: https://webrtcglossary.com/stun
    /// [TURN]: https://webrtcglossary.com/turn
    pub r#static: HashMap<String, Server>,
}

/// Possible kinds of [`Ice`] servers.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Managed [`Coturn`] server.
    #[default]
    Coturn,

    /// List of unmanaged [STUN]/[TURN] [`Server`]s.
    ///
    /// [STUN]: https://webrtcglossary.com/stun
    /// [TURN]: https://webrtcglossary.com/turn
    Static,
}

/// Unmanaged [STUN]/[TURN] server settings.
///
/// [STUN]: https://webrtcglossary.com/stun
/// [TURN]: https://webrtcglossary.com/turn
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Server {
    /// URLs of this [TURN]/[STUN] server.
    ///
    /// [STUN]: https://webrtcglossary.com/stun
    /// [TURN]: https://webrtcglossary.com/turn
    #[serde(deserialize_with = "Server::parse_urls")]
    pub urls: Vec<Cow<'static, str>>,

    /// Username to use during the authentication process.
    pub user: Option<Cow<'static, str>>,

    /// The credential to use when logging into the server.
    pub pass: Option<Cow<'static, str>>,
}

impl Server {
    /// Parses [`Server::urls`] from the provided [`Deserializer`] as CSV
    /// (comma-separated values) string.
    ///
    /// # Errors
    ///
    /// - If cannot parse CSV strings.
    /// - If parsed [`Server::urls`] is empty or contains empty values.
    fn parse_urls<'de, D>(d: D) -> Result<Vec<Cow<'static, str>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde_json as json;

        let out: Vec<Cow<'static, str>> = match json::Value::deserialize(d)? {
            json::Value::String(urls) => urls
                .split(',')
                .map(|u| u.trim().to_owned().into())
                .collect(),
            json::Value::Array(list) => {
                let mut out = Vec::new();
                for val in list {
                    match val {
                        json::Value::String(urls) => out.extend(
                            urls.split(',').map(|u| u.trim().to_owned().into()),
                        ),
                        _ => return Err(D::Error::custom("Unexpected value")),
                    }
                }
                out
            }
            _ => return Err(D::Error::custom("Unexpected value")),
        };

        if out.is_empty() || out.iter().any(|url| url.is_empty()) {
            return Err(D::Error::custom("Empty values are not allowed"));
        }

        Ok(out)
    }
}

/// Managed [Coturn] server settings.
///
/// [Coturn]: https://github.com/coturn/coturn
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct Coturn {
    /// Host of [Coturn] server.
    ///
    /// Defaults to `localhost`.
    ///
    /// [Coturn]: https://github.com/coturn/coturn
    #[default = "localhost"]
    pub host: Cow<'static, str>,

    /// [TURN] port of [Coturn] server.
    ///
    /// Defaults to `3478`.
    ///
    /// [Coturn]: https://github.com/coturn/coturn
    /// [TURN]: https://webrtcglossary.com/turn
    #[default = 3478]
    pub port: u16,

    /// Name of static user to authenticate on [Coturn] server as.
    ///
    /// Defaults to `USER`.
    ///
    /// [Coturn]: https://github.com/coturn/coturn
    #[default = "USER"]
    pub user: Cow<'static, str>,

    /// Password of static user to authenticate on [Coturn] server with.
    ///
    /// Defaults to `PASS`.
    ///
    /// [Coturn]: https://github.com/coturn/coturn
    #[default = "PASS"]
    pub pass: Cow<'static, str>,

    /// Database settings
    pub db: Db,

    /// Admin interface settings.
    pub cli: CoturnCli,
}

impl Coturn {
    /// Builds [`String`] address of this [`Coturn`] server out of its
    /// [`Coturn::host`] and [`Coturn::port`].
    #[inline]
    #[must_use]
    pub fn addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

/// Settings of databases which back [Coturn] storage.
///
/// [Coturn]: https://github.com/coturn/coturn
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct Db {
    /// [Redis] database settings.
    ///
    /// [Redis]: https://redis.io
    pub redis: Redis,
}

/// Settings of [Redis] database server which backs [Coturn] storage.
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

    use crate::{conf::Conf, overrided_by_env_conf, try_overrided_by_env_conf};

    use super::*;

    #[test]
    #[serial]
    fn redis_db_overrides_defaults() {
        let default_conf = Conf::default();
        let env_conf = overrided_by_env_conf!(
            "MEDEA_ICE__COTURN__DB__REDIS__HOST" => "5.5.5.5",
            "MEDEA_ICE__COTURN__DB__REDIS__PORT" => "1234",
            "MEDEA_ICE__COTURN__DB__REDIS__PASS" => "hellofellow",
            "MEDEA_ICE__COTURN__DB__REDIS__DB_NUMBER" => "10",
            "MEDEA_ICE__COTURN__DB__REDIS__CONNECT_TIMEOUT" => "10s",
        );

        assert_ne!(
            default_conf.ice.coturn.db.redis.host,
            env_conf.ice.coturn.db.redis.host,
        );
        assert_ne!(
            default_conf.ice.coturn.db.redis.port,
            env_conf.ice.coturn.db.redis.port,
        );
        assert_ne!(
            default_conf.ice.coturn.db.redis.pass,
            env_conf.ice.coturn.db.redis.pass,
        );
        assert_ne!(
            default_conf.ice.coturn.db.redis.db_number,
            env_conf.ice.coturn.db.redis.db_number,
        );
        assert_ne!(
            default_conf.ice.coturn.db.redis.connect_timeout,
            env_conf.ice.coturn.db.redis.connect_timeout,
        );

        assert_eq!(env_conf.ice.coturn.db.redis.host, "5.5.5.5");
        assert_eq!(env_conf.ice.coturn.db.redis.port, 1234);
        assert_eq!(
            env_conf.ice.coturn.db.redis.connect_timeout,
            Duration::from_secs(10),
        );
    }

    #[test]
    #[serial]
    fn coturn_overrides_defaults() {
        let default_conf = Conf::default();
        let env_conf = overrided_by_env_conf!(
            "MEDEA_ICE__COTURN__HOST" => "example.com",
            "MEDEA_ICE__COTURN__PORT" => "1234",
            "MEDEA_ICE__COTURN__USER" => "ferris",
            "MEDEA_ICE__COTURN__PASS" => "qwerty",
        );

        assert_ne!(default_conf.ice.coturn.host, env_conf.ice.coturn.host);
        assert_ne!(default_conf.ice.coturn.port, env_conf.ice.coturn.port);
        assert_ne!(default_conf.ice.coturn.user, env_conf.ice.coturn.user);
        assert_ne!(default_conf.ice.coturn.pass, env_conf.ice.coturn.pass);

        assert_eq!(env_conf.ice.coturn.host, "example.com");
        assert_eq!(env_conf.ice.coturn.port, 1234);
        assert_eq!(env_conf.ice.coturn.addr(), "example.com:1234");
    }

    #[test]
    #[serial]
    fn coturn_cli() {
        let default_conf = Conf::default();
        let env_conf = overrided_by_env_conf!(
            "MEDEA_ICE__COTURN__CLI__HOST" => "4.4.4.4",
            "MEDEA_ICE__COTURN__CLI__PORT" => "1234",
            "MEDEA_ICE__COTURN__CLI__PASS" => "clipass",
        );

        assert_ne!(
            default_conf.ice.coturn.cli.host,
            env_conf.ice.coturn.cli.host,
        );
        assert_ne!(
            default_conf.ice.coturn.cli.port,
            env_conf.ice.coturn.cli.port,
        );
        assert_ne!(
            default_conf.ice.coturn.cli.pass,
            env_conf.ice.coturn.cli.pass,
        );

        assert_eq!(env_conf.ice.coturn.cli.host, "4.4.4.4");
        assert_eq!(env_conf.ice.coturn.cli.port, 1234);
        assert_eq!(env_conf.ice.coturn.cli.pass, "clipass");
    }

    #[test]
    #[serial]
    fn coturn_cli_pool() {
        let default_conf = Conf::default();
        let env_conf = overrided_by_env_conf!(
            "MEDEA_ICE__COTURN__CLI__POOL__MAX_SIZE" => "10",
            "MEDEA_ICE__COTURN__CLI__POOL__WAIT_TIMEOUT" => "1s",
            "MEDEA_ICE__COTURN__CLI__POOL__CONNECT_TIMEOUT" => "4s",
            "MEDEA_ICE__COTURN__CLI__POOL__RECYCLE_TIMEOUT" => "3s",
        );

        assert_ne!(
            default_conf.ice.coturn.cli.pool.max_size,
            env_conf.ice.coturn.cli.pool.max_size,
        );
        assert_ne!(
            default_conf.ice.coturn.cli.pool.wait_timeout,
            env_conf.ice.coturn.cli.pool.wait_timeout,
        );
        assert_ne!(
            default_conf.ice.coturn.cli.pool.connect_timeout,
            env_conf.ice.coturn.cli.pool.connect_timeout,
        );
        assert_ne!(
            default_conf.ice.coturn.cli.pool.recycle_timeout,
            env_conf.ice.coturn.cli.pool.recycle_timeout,
        );

        assert_eq!(env_conf.ice.coturn.cli.pool.max_size, 10);
        assert_eq!(
            env_conf.ice.coturn.cli.pool.wait_timeout,
            Duration::from_secs(1),
        );
        assert_eq!(
            env_conf.ice.coturn.cli.pool.connect_timeout,
            Duration::from_secs(4),
        );
        assert_eq!(
            env_conf.ice.coturn.cli.pool.recycle_timeout,
            Duration::from_secs(3),
        );
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

    #[test]
    #[serial]
    fn static_servers() {
        let conf = overrided_by_env_conf!(
            "MEDEA_ICE__STATIC__STUN__URLS" => "stun:stunserver.example.org",
            "MEDEA_ICE__STATIC__TURN__URLS" =>
                "turn:turnserver.example.org, turn:turnserver2.example.org",
            "MEDEA_ICE__STATIC__TURN__USER" => "webrtc",
            "MEDEA_ICE__STATIC__TURN__PASS" => "password",
        );

        assert_eq!(conf.ice.r#static.len(), 2);
        assert!(conf.ice.r#static.contains_key("stun"));
        assert!(conf.ice.r#static.contains_key("turn"));

        let stun = &conf.ice.r#static["stun"];
        assert_eq!(stun.urls.len(), 1);
        assert_eq!(stun.urls[0], "stun:stunserver.example.org");
        assert_eq!(stun.user, None);
        assert_eq!(stun.pass, None);

        let turn = &conf.ice.r#static["turn"];
        assert_eq!(turn.urls.len(), 2);
        assert_eq!(turn.urls[0], "turn:turnserver.example.org");
        assert_eq!(turn.urls[1], "turn:turnserver2.example.org");
        assert_eq!(turn.user.as_deref(), Some("webrtc"));
        assert_eq!(turn.pass.as_deref(), Some("password"));
    }

    #[test]
    #[serial]
    fn disallows_empty_static_server_urls() {
        let conf = try_overrided_by_env_conf!(
            "MEDEA_ICE__STATIC__STUN__URLS" => "",
        );

        assert!(conf.is_err());

        let conf = try_overrided_by_env_conf!(
            "MEDEA_ICE__STATIC__TURN__URLS" => "turn:turnserver.example.org,",
        );

        assert!(conf.is_err());
    }
}

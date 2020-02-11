//! STUN/TURN server settings.

use std::{
    borrow::Cow,
    net::{IpAddr, Ipv4Addr},
    time::Duration,
};

use deadpool::managed::{
    PoolConfig as DeadpoolPoolConfig, Timeouts as DeadpoolTimeouts,
};
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

/// STUN/TURN server settings.
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct Turn {
    /// Database settings
    pub db: Db,
    /// Coturn telnet connection settings.
    pub cli: CoturnCli,
    /// Host of STUN/TURN server. Defaults to `localhost`.
    #[default = "localhost"]
    pub host: Cow<'static, str>,
    /// Port to connect TURN server. Defaults to `3478`.
    #[default = 3478]
    pub port: u16,
    /// Username for authorize on TURN server.
    #[default(String::from("USER"))]
    pub user: String,
    /// Password for authorize on TURN server.
    #[default(String::from("PASS"))]
    pub pass: String,
}

impl Turn {
    /// Builds [`String`] addr from `host` and `port`.
    #[inline]
    pub fn addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct Db {
    /// Redis server settings.
    pub redis: Redis,
}

/// Setting of [Redis] server which used by [coturn].
///
/// [Redis]: https://redis.io/
/// [coturn]: https://github.com/coturn/coturn
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct Redis {
    /// IP address Redis server. Defaults to `127.0.0.1`.
    #[default(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)))]
    pub ip: IpAddr,
    /// Port to connect Redis server. Defaults to `6379`.
    #[default = 6379]
    pub port: u16,
    /// Password for authorize on Redis server.
    #[default(String::from("turn"))]
    pub pass: String,
    /// The database number to use. This is usually 0.
    #[default = 0]
    pub db_number: i64,
    // TODO: replace with PoolConfig
    /// The duration to wait to start a connection before returning err.
    #[default(Duration::from_secs(5))]
    #[serde(with = "humantime_serde")]
    pub connection_timeout: Duration,
}

/// Settings of [Coturn] server telnet interface.
///
/// [Coturn]: https://github.com/coturn/coturn
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct CoturnCli {
    /// Coturn server cli IP address. Defaults to `127.0.0.1`.
    #[default(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)))]
    pub ip: IpAddr,
    /// Coturn server cli port. Defaults to `5766`.
    #[default = 5766]
    pub port: u16,
    /// Password for authorize on Coturn server telnet interface.
    #[default(String::from("turn"))]
    pub pass: String,
    /// Connection pool config.
    pub pool: PoolConfig,
}

/// [Deadpool] connection pool config.
///
/// [Deadpool]: https://crates.io/crates/deadpool
#[derive(Copy, Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct PoolConfig {
    /// Maximum size of the pool
    #[default = 16]
    pub max_size: usize,
    /// Timeout when waiting for available connection to become available.
    #[default(Some(Duration::from_secs(5)))]
    #[serde(with = "humantime_serde")]
    pub wait: Option<Duration>,
    /// Timeout when creating a new connection.
    #[default(Some(Duration::from_secs(5)))]
    #[serde(with = "humantime_serde")]
    pub create: Option<Duration>,
    /// Timeout when recycling connection.
    #[default(Some(Duration::from_secs(5)))]
    #[serde(with = "humantime_serde")]
    pub recycle: Option<Duration>,
}

impl Into<DeadpoolPoolConfig> for PoolConfig {
    fn into(self) -> DeadpoolPoolConfig {
        let wait = self.wait.and_then(|wait| {
            if wait.as_nanos() == 0 {
                None
            } else {
                Some(wait)
            }
        });
        let create = self.create.and_then(|create| {
            if create.as_nanos() == 0 {
                None
            } else {
                Some(create)
            }
        });
        let recycle = self.recycle.and_then(|recycle| {
            if recycle.as_nanos() == 0 {
                None
            } else {
                Some(recycle)
            }
        });

        DeadpoolPoolConfig {
            max_size: self.max_size,
            timeouts: DeadpoolTimeouts {
                wait,
                create,
                recycle,
            },
        }
    }
}

#[cfg(test)]
mod spec {
    use std::{net::Ipv4Addr, time::Duration};

    use serial_test_derive::serial;

    use crate::{conf::Conf, overrided_by_env_conf};

    use super::*;

    #[test]
    #[serial]
    fn redis_db_overrides_defaults() {
        let default_conf = Conf::default();
        let env_conf = overrided_by_env_conf!(
            "MEDEA_TURN__DB__REDIS__IP" => "5.5.5.5",
            "MEDEA_TURN__DB__REDIS__PORT" => "1234",
            "MEDEA_TURN__DB__REDIS__PASS" => "hellofellow",
            "MEDEA_TURN__DB__REDIS__DB_NUMBER" => "10",
            "MEDEA_TURN__DB__REDIS__CONNECTION_TIMEOUT" => "10s",
        );

        assert_ne!(default_conf.turn.db.redis.ip, env_conf.turn.db.redis.ip);
        assert_ne!(
            default_conf.turn.db.redis.port,
            env_conf.turn.db.redis.port
        );
        assert_ne!(
            default_conf.turn.db.redis.pass,
            env_conf.turn.db.redis.pass
        );
        assert_ne!(
            default_conf.turn.db.redis.db_number,
            env_conf.turn.db.redis.db_number
        );
        assert_ne!(
            default_conf.turn.db.redis.connection_timeout,
            env_conf.turn.db.redis.connection_timeout
        );

        assert_eq!(env_conf.turn.db.redis.ip, Ipv4Addr::new(5, 5, 5, 5));
        assert_eq!(env_conf.turn.db.redis.port, 1234);
        assert_eq!(
            env_conf.turn.db.redis.connection_timeout,
            Duration::from_secs(10)
        );
    }

    #[test]
    #[serial]
    fn overrides_defaults() {
        let default_conf = Conf::default();
        let env_conf = overrided_by_env_conf!(
            "MEDEA_TURN__HOST" => "example.com",
            "MEDEA_TURN__PORT" => "1234",
            "MEDEA_TURN__USER" => "ferris",
            "MEDEA_TURN__PASS" => "qwerty",
        );

        assert_ne!(default_conf.turn.host, env_conf.turn.host);
        assert_ne!(default_conf.turn.port, env_conf.turn.port);
        assert_ne!(default_conf.turn.user, env_conf.turn.user);
        assert_ne!(default_conf.turn.pass, env_conf.turn.pass);

        assert_eq!(env_conf.turn.host, "example.com");
        assert_eq!(env_conf.turn.port, 1234);
        assert_eq!(env_conf.turn.addr(), "example.com:1234");
    }

    #[test]
    #[serial]
    fn coturn_cli() {
        let default_conf = Conf::default();
        let env_conf = overrided_by_env_conf!(
            "MEDEA_TURN__CLI__IP" => "4.4.4.4",
            "MEDEA_TURN__CLI__PORT" => "1234",
            "MEDEA_TURN__CLI__PASS" => "clipass",
        );

        assert_ne!(default_conf.turn.cli.ip, env_conf.turn.cli.ip);
        assert_ne!(default_conf.turn.cli.port, env_conf.turn.cli.port);
        assert_ne!(default_conf.turn.cli.pass, env_conf.turn.cli.pass);

        assert_eq!(env_conf.turn.cli.ip, Ipv4Addr::new(4, 4, 4, 4));
        assert_eq!(env_conf.turn.cli.port, 1234);
        assert_eq!(env_conf.turn.cli.pass, "clipass");
    }

    #[test]
    #[serial]
    fn coturn_cli_pool() {
        let default_conf = Conf::default();
        let env_conf = overrided_by_env_conf!(
            "MEDEA_TURN__CLI__POOL__MAX_SIZE" => "10",
            "MEDEA_TURN__CLI__POOL__WAIT" => "1s",
            "MEDEA_TURN__CLI__POOL__CREATE" => "2s",
            "MEDEA_TURN__CLI__POOL__RECYCLE" => "3s",
        );

        assert_ne!(
            default_conf.turn.cli.pool.max_size,
            env_conf.turn.cli.pool.max_size
        );
        assert_ne!(
            default_conf.turn.cli.pool.wait,
            env_conf.turn.cli.pool.wait
        );
        assert_ne!(
            default_conf.turn.cli.pool.create,
            env_conf.turn.cli.pool.create
        );
        assert_ne!(
            default_conf.turn.cli.pool.recycle,
            env_conf.turn.cli.pool.recycle
        );

//        assert_eq!(env_conf.turn.cli.pool.max_size, 10);
//        assert_eq!(env_conf.turn.cli.pool.wait, Some(Duration::from_secs(1)));
//        assert_eq!(env_conf.turn.cli.pool.create, Some(Duration::from_secs(2)));
//        assert_eq!(
//            env_conf.turn.cli.pool.recycle,
//            Some(Duration::from_secs(3))
//        );
    }

    #[test]
    fn into_pool_config() {
        let pool_config = PoolConfig {
            max_size: 6,
            wait: None,
            create: Some(Duration::from_secs(0)),
            recycle: Some(Duration::from_secs(2)),
        };

        let pool_config: DeadpoolPoolConfig = pool_config.into();

        assert_eq!(pool_config.max_size, 6);
        assert!(pool_config.timeouts.wait.is_none());
        assert!(pool_config.timeouts.create.is_none());
        assert_eq!(pool_config.timeouts.recycle, Some(Duration::from_secs(2)));
    }
}

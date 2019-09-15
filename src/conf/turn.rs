//! STUN/TURN server settings.

use std::{
    borrow::Cow,
    net::{IpAddr, Ipv4Addr},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

/// STUN/TURN server settings.
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct Turn {
    /// Database settings
    pub db: Db,
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
    /// The duration to wait to start a connection before returning err.
    #[default(Duration::from_secs(5))]
    #[serde(with = "humantime_serde")]
    pub connection_timeout: Duration,
}

#[cfg(test)]
mod turn_conf_specs {
    use std::env;

    use serial_test_derive::serial;

    use crate::conf::Conf;

    #[test]
    #[serial]
    fn overrides_defaults() {
        let default_conf = Conf::default();

        env::set_var("MEDEA_TURN__DB__REDIS__IP", "0.0.0.0");
        env::set_var("MEDEA_TURN__DB__REDIS__PORT", "4444");
        env::set_var("MEDEA_TURN__DB__REDIS__PASS", "hellofellow");
        env::set_var("MEDEA_TURN__DB__REDIS__DB_NUMBER", "10");
        env::set_var("MEDEA_TURN__DB__REDIS__CONNECTION_TIMEOUT", "10s");
        env::set_var("MEDEA_TURN__HOST", "example.com");
        env::set_var("MEDEA_TURN__PORT", "4444");
        env::set_var("MEDEA_TURN__USER", "ferris");
        env::set_var("MEDEA_TURN__PASS", "qwerty");
        let env_conf = Conf::parse().unwrap();
        env::remove_var("MEDEA_TURN__DB__REDIS__IP");
        env::remove_var("MEDEA_TURN__DB__REDIS__PORT");
        env::remove_var("MEDEA_TURN__DB__REDIS__PASS");
        env::remove_var("MEDEA_TURN__DB__REDIS__DB_NUMBER");
        env::remove_var("MEDEA_TURN__DB__REDIS__CONNECTION_TIMEOUT");
        env::remove_var("MEDEA_TURN__HOST");
        env::remove_var("MEDEA_TURN__PORT");
        env::remove_var("MEDEA_TURN__USER");
        env::remove_var("MEDEA_TURN__PASS");

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
        assert_ne!(default_conf.turn.host, env_conf.turn.host);
        assert_ne!(default_conf.turn.port, env_conf.turn.port);
        assert_ne!(default_conf.turn.user, env_conf.turn.user);
        assert_ne!(default_conf.turn.pass, env_conf.turn.pass);
    }
}

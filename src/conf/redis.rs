//! Redis server settings.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs as _};

use serde::{Deserialize, Serialize};
use smart_default::*;

/// Redis server settings.
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
pub struct Redis {
    /// IP address Redis server. Defaults to `0.0.0.0`.
    #[default(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)))]
    pub ip: IpAddr,
    /// Port to connect Redis server. Defaults to `6379`.
    #[default(6379)]
    pub port: u16,
    /// Password for authorize on Redis server.
    #[default(String::from("pass"))]
    pub pass: String,
}

impl Redis {
    /// Builds [`SocketAddr`] from `ip` and `port`.
    #[inline]
    pub fn get_addr(&self) -> SocketAddr {
        (self.ip, self.port)
            .to_socket_addrs()
            .unwrap()
            .next()
            .unwrap()
    }
}

#[cfg(test)]
mod redis_spec {
    use std::env;

    use serial_test_derive::serial;

    use crate::conf::Conf;

    use super::*;

    #[test]
    #[serial]
    fn overrides_defaults_and_gets_addr() {
        let default_conf = Conf::default();

        env::set_var("MEDEA_REDIS.IP", "5.5.5.5");
        env::set_var("MEDEA_REDIS.PORT", "1234");

        let env_conf = Conf::parse().unwrap();

        assert_ne!(default_conf.redis.ip, env_conf.redis.ip);
        assert_ne!(default_conf.redis.port, env_conf.redis.port);

        assert_eq!(env_conf.redis.ip, Ipv4Addr::new(5, 5, 5, 5));
        assert_eq!(env_conf.redis.port, 1234);
        assert_eq!(env_conf.redis.get_addr(), "5.5.5.5:1234".parse().unwrap(),);
    }
}

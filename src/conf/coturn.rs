//! COTURN server settings.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs as _};

use serde::{Deserialize, Serialize};
use smart_default::*;

/// COTURN server settings.
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
pub struct Coturn {
    /// IP address COTURN server. Defaults to `0.0.0.0`.
    #[default(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)))]
    pub ip: IpAddr,
    /// Port to connect COTURN server. Defaults to `3478`.
    #[default(3478)]
    pub port: u16,
    /// Username for authorize on COTURN server.
    pub user: String,
    /// Password for authorize on COTURN server.
    pub pass: String,
}

impl Coturn {
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
mod coturn_spec {
    use std::env;

    use serial_test_derive::serial;

    use crate::conf::Conf;

    use super::*;

    #[test]
    #[serial]
    fn overrides_defaults_and_gets_addr() {
        let default_conf = Conf::default();

        env::set_var("MEDEA_COTURN.IP", "5.5.5.5");
        env::set_var("MEDEA_COTURN.PORT", "1234");

        let env_conf = Conf::parse().unwrap();

        assert_ne!(default_conf.coturn.ip, env_conf.coturn.ip);
        assert_ne!(default_conf.coturn.port, env_conf.coturn.port);

        assert_eq!(env_conf.coturn.ip, Ipv4Addr::new(5, 5, 5, 5));
        assert_eq!(env_conf.coturn.port, 1234);
        assert_eq!(
            env_conf.coturn.get_addr(),
            "5.5.5.5:1234".parse().unwrap(),
        );
    }
}

//! COTURN server settings.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs as _};

use serde::{Deserialize, Serialize};
use smart_default::*;

/// COTURN server settings.
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
pub struct Coturn {
    /// IP address to bind COTURN server to. Defaults to `0.0.0.0`.
    #[default(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)))]
    pub bind_ip: IpAddr,
    /// Port to bind COTURN server to. Defaults to `3478`.
    #[default(3478)]
    pub bind_port: u16,
    /// Username for authorize on COTURN server.
    pub user: String,
    /// Password for authorize on COTURN server.
    pub pass: String,
}

impl Coturn {
    /// Builds [`SocketAddr`] from `bind_ip` and `bind_port`.
    #[inline]
    pub fn get_bind_addr(&self) -> SocketAddr {
        (self.bind_ip, self.bind_port)
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
    fn overrides_defaults_and_gets_bind_addr() {
        let default_conf = Conf::default();

        env::set_var("MEDEA_COTURN.BIND_IP", "5.5.5.5");
        env::set_var("MEDEA_COTURN.BIND_PORT", "1234");

        let env_conf = Conf::parse().unwrap();

        assert_ne!(default_conf.coturn.bind_ip, env_conf.coturn.bind_ip);
        assert_ne!(default_conf.coturn.bind_port, env_conf.coturn.bind_port);

        assert_eq!(env_conf.coturn.bind_ip, Ipv4Addr::new(5, 5, 5, 5));
        assert_eq!(env_conf.coturn.bind_port, 1234);
        assert_eq!(
            env_conf.coturn.get_bind_addr(),
            "5.5.5.5:1234".parse().unwrap(),
        );
    }
}

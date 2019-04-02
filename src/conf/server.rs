use serde::{Deserialize, Serialize};
use smart_default::*;

use std::net::{IpAddr, Ipv4Addr};

/// Server represents [`Server`] configuration section.
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
pub struct Server {
    /// IP to bind to.
    #[default(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)))]
    pub bind_ip: IpAddr,
    /// Port to bind to.
    #[default(8080)]
    pub bind_port: u16,
}

impl Server {
    /// Builds ['std::net::ToSocketAddrs'] from ['bind_ip'] and ['bind_port']
    pub fn get_bind_addr(&self) -> impl std::net::ToSocketAddrs {
        (self.bind_ip, self.bind_port)
    }
}

#[cfg(test)]
mod test {
    use serial_test_derive::serial;

    use std::net::{Ipv4Addr, SocketAddr, ToSocketAddrs as _};

    use crate::conf::Conf;

    #[test]
    #[serial]
    fn override_defaults_and_get_bind_addr() {
        let default_conf = Conf::default();

        std::env::set_var("MEDEA_SERVER.BIND_IP", "5.5.5.5");
        std::env::set_var("MEDEA_SERVER.BIND_PORT", "1234");

        let env_conf = Conf::parse().unwrap();

        assert_ne!(default_conf.server.bind_ip, env_conf.server.bind_ip);
        assert_ne!(default_conf.server.bind_port, env_conf.server.bind_port);

        assert_eq!(env_conf.server.bind_ip, Ipv4Addr::new(5, 5, 5, 5));
        assert_eq!(env_conf.server.bind_port, 1234);

        let addr: SocketAddr = env_conf
            .server
            .get_bind_addr()
            .to_socket_addrs()
            .unwrap()
            .into_iter()
            .next()
            .unwrap();

        assert_eq!(addr, "5.5.5.5:1234".parse().unwrap());
    }
}

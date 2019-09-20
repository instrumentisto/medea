//! HTTP server settings.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs as _};

use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

/// HTTP server settings.
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct HttpListener {
    /// IP address to bind HTTP server to. Defaults to `0.0.0.0`.
    #[default(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)))]
    pub bind_ip: IpAddr,

    /// Port to bind HTTP server to. Defaults to `8080`.
    #[default(8080)]
    pub bind_port: u16,
}

impl HttpListener {
    /// Builds [`SocketAddr`] from `bind_ip` and `bind_port`.
    #[inline]
    pub fn bind_addr(&self) -> SocketAddr {
        (self.bind_ip, self.bind_port)
            .to_socket_addrs()
            .unwrap()
            .next()
            .unwrap()
    }
}

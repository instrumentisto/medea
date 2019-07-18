//! STUN/TURN server settings.

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs as _},
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
    /// IP address STUN/TURN server. Defaults to `127.0.0.1`.
    #[default(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)))]
    pub ip: IpAddr,
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
    /// Builds [`SocketAddr`] from `ip` and `port`.
    #[inline]
    pub fn addr(&self) -> SocketAddr {
        (self.ip, self.port)
            .to_socket_addrs()
            .unwrap()
            .next()
            .unwrap()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct Db {
    /// Redis server settings.
    pub redis: Redis,
}

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
    #[serde(with = "serde_humantime")]
    pub connection_timeout: Duration,
}

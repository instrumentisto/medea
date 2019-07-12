//! gRPC server settings.

use std::net::{IpAddr, Ipv4Addr};

use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

/// gRPC server settings.
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct Grpc {
    /// IP address to bind gRPC server to. Defaults to `0.0.0.0`.
    #[default(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)))]
    pub bind_ip: IpAddr,

    /// Port to bind gRPC server to. Defaults to `50_051`.
    #[default(50_051)]
    pub bind_port: u16,

    /// Completion queue count of gRPC server. Defaults to `2`.
    #[default(2)]
    pub completion_queue_count: usize,
}

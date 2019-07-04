use std::net::{IpAddr, Ipv4Addr};

use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct Grpc {
    /// IP address to bind gRPC server to. Defaults to `0.0.0.0`.
    #[default(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)))]
    pub bind_ip: IpAddr,

    /// Port to bind gRPC server to. Defaults to `8080`.
    #[default(50_051)]
    pub bind_port: u16,
}

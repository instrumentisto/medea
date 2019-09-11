//! [Control API] gRPC server settings.
//!
//! [Control API]: http://tiny.cc/380uaz

use std::net::{IpAddr, Ipv4Addr};

use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

/// [Control API] gRPC server settings.
///
/// [Control API]: http://tiny.cc/380uaz
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct ControlApiGrpcServer {
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

#[cfg(test)]
mod control_grpc_conf_specs {
    use std::env;

    use serial_test_derive::serial;

    use crate::conf::Conf;

    #[test]
    #[serial]
    fn overrides_defaults() {
        let default_conf = Conf::default();

        env::set_var("MEDEA_CONTROL.GRPC.BIND_IP", "127.0.0.1");
        env::set_var("MEDEA_CONTROL.GRPC.BIND_PORT", "44444");
        env::set_var("MEDEA_CONTROL.GRPC.COMPLETION_QUEUE_COUNT", "10");
        let env_conf = Conf::parse().unwrap();
        env::remove_var("MEDEA_CONTROL.GRPC.BIND_IP");
        env::remove_var("MEDEA_CONTROL.GRPC.BIND_PORT");
        env::remove_var("MEDEA_CONTROL.GRPC.COMPLETION_QUEUE_COUNT");

        assert_ne!(
            default_conf.control.grpc.bind_ip,
            env_conf.control.grpc.bind_ip
        );
        assert_ne!(
            default_conf.control.grpc.bind_port,
            env_conf.control.grpc.bind_port
        );
        assert_ne!(
            default_conf.control.grpc.completion_queue_count,
            env_conf.control.grpc.completion_queue_count
        );
    }
}

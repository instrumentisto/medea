//! Settings for application servers.

use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

use super::{grpc_listener::GrpcListener, http_listener::HttpListener};

/// [Client API] servers settings.
///
/// [Client API]: http://tiny.cc/c80uaz
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct ClientApiServer {
    /// [Client API] server settings.
    ///
    /// [Client API]: http://tiny.cc/c80uaz
    pub http: HttpListener,

    /// Public URL of server. Address for exposed [Client API].
    ///
    /// This address will be returned from [Control API] in `sids` and to
    /// this address will connect [Jason] for start session.
    ///
    /// Defaults to `ws://0.0.0.0:8080`.
    ///
    /// [Client API]: http://tiny.cc/c80uaz
    /// [Jason]: https://github.com/instrumentisto/medea/tree/master/jason
    #[default("ws://0.0.0.0:8080".to_string())]
    pub public_url: String,
}

/// [Control API] servers settings.
///
/// [Control API]: http://tiny.cc/380uaz
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct ControlApiServer {
    /// gRPC [Control API] server settings.
    ///
    /// [Control API]: http://tiny.cc/380uaz
    pub grpc: GrpcListener,
}

/// Settings for application servers.
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct Server {
    /// [Client API] servers settings.
    ///
    /// [Client API]: http://tiny.cc/c80uaz
    pub client: ClientApiServer,

    /// [Control API] servers settings.
    ///
    /// [Control API]: http://tiny.cc/380uaz
    pub control: ControlApiServer,
}

#[cfg(test)]
mod server_spec {
    use std::{env, net::Ipv4Addr};

    use serial_test_derive::serial;

    use crate::conf::Conf;

    #[test]
    #[serial]
    fn overrides_defaults_and_gets_bind_addr() {
        let default_conf = Conf::default();

        env::set_var("MEDEA_SERVER__CLIENT__HTTP__BIND_IP", "5.5.5.5");
        env::set_var("MEDEA_SERVER__CLIENT__HTTP__BIND_PORT", "1234");

        let env_conf = Conf::parse().unwrap();

        env::remove_var("MEDEA_SERVER__CLIENT__HTTP__BIND_IP");
        env::remove_var("MEDEA_SERVER__CLIENT__HTTP__BIND_PORT");

        assert_ne!(
            default_conf.server.client.http.bind_ip,
            env_conf.server.client.http.bind_ip
        );
        assert_ne!(
            default_conf.server.client.http.bind_port,
            env_conf.server.client.http.bind_port
        );

        assert_eq!(
            env_conf.server.client.http.bind_ip,
            Ipv4Addr::new(5, 5, 5, 5)
        );
        assert_eq!(env_conf.server.client.http.bind_port, 1234);
        assert_eq!(
            env_conf.server.client.http.bind_addr(),
            "5.5.5.5:1234".parse().unwrap(),
        );
    }
}

#[cfg(test)]
mod control_grpc_conf_specs {
    use std::net::Ipv4Addr;

    use serial_test_derive::serial;

    use crate::{conf::Conf, overrided_by_env_conf};

    #[test]
    #[serial]
    fn overrides_defaults() {
        let default_conf = Conf::default();
        let env_conf = overrided_by_env_conf!(
            "MEDEA_SERVER__CONTROL__GRPC__BIND_IP" => "182.98.12.48",
            "MEDEA_SERVER__CONTROL__GRPC__BIND_PORT" => "44444",
            "MEDEA_SERVER__CONTROL__GRPC__COMPLETION_QUEUE_COUNT" => "10"
        );

        assert_ne!(
            default_conf.server.control.grpc.bind_ip,
            env_conf.server.control.grpc.bind_ip
        );
        assert_ne!(
            default_conf.server.control.grpc.bind_port,
            env_conf.server.control.grpc.bind_port
        );
        assert_ne!(
            default_conf.server.control.grpc.completion_queue_count,
            env_conf.server.control.grpc.completion_queue_count
        );

        assert_eq!(env_conf.server.control.grpc.completion_queue_count, 10);
        assert_eq!(env_conf.server.control.grpc.bind_port, 44444);
        assert_eq!(
            env_conf.server.control.grpc.bind_ip,
            Ipv4Addr::new(182, 98, 12, 48)
        );
    }
}

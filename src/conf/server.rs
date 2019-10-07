//! Settings for application servers.

use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

use super::{grpc_listener::GrpcListener, http_listener::HttpListener};

/// [Client API] servers settings.
///
/// [Client API]: https://tinyurl.com/yx9thsnr
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct ClientApiServer {
    /// [Client API] server settings.
    ///
    /// [Client API]: https://tinyurl.com/yx9thsnr
    pub http: HttpListener,

    /// Public URL of server. Address for exposed [Client API].
    ///
    /// This address will be returned from [Control API] in `sids` and to
    /// this address will connect [Jason] for start session.
    ///
    /// This address and address to which [Medea]'s RPC server will be bound
    /// may be different. Address to which RPC server will be bound always
    /// `{{ MEDEA_SERVER__CLIENT__HTTP__BIND_IP }}:{{
    /// MEDEA_SERVER__CLIENT__HTTP__BIND_PORT }}/ws`.
    ///
    /// This is needed for flexibility in web proxy configuration. For example,
    /// if you want to set address to which users will connect you may set
    /// proxying in nginx config from address `wss://example.com/websocket`
    /// to `ws://0.0.0.0:8080/ws` ([Medea] RPC server). In this case
    /// you should set this value to `wss://example.com/websocket`.
    ///
    /// Defaults to `ws://0.0.0.0:8080/ws`.
    ///
    /// [Client API]: https://tinyurl.com/yx9thsnr
    /// [Jason]: https://github.com/instrumentisto/medea/tree/master/jason
    /// [Medea]: https://github.com/instrumentisto/medea
    #[default("ws://0.0.0.0:8080/ws".to_string())]
    pub public_url: String,
}

/// [Control API] servers settings.
///
/// [Control API]: https://tinyurl.com/yxsqplq7
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct ControlApiServer {
    /// gRPC [Control API] server settings.
    ///
    /// [Control API]: https://tinyurl.com/yxsqplq7
    pub grpc: GrpcListener,
}

/// Settings for application servers.
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct Server {
    /// [Client API] servers settings.
    ///
    /// [Client API]: https://tinyurl.com/yx9thsnr
    pub client: ClientApiServer,

    /// [Control API] servers settings.
    ///
    /// [Control API]: https://tinyurl.com/yxsqplq7
    pub control: ControlApiServer,
}

#[cfg(test)]
mod server_spec {
    use std::net::Ipv4Addr;

    use serial_test_derive::serial;

    use crate::{conf::Conf, overrided_by_env_conf};

    #[test]
    #[serial]
    fn overrides_defaults_and_gets_bind_addr() {
        let default_conf = Conf::default();

        let env_conf = overrided_by_env_conf!(
            "MEDEA_SERVER__CLIENT__HTTP__BIND_IP" => "5.5.5.5",
            "MEDEA_SERVER__CLIENT__HTTP__BIND_PORT" => "1234",
        );

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
            "MEDEA_SERVER__CONTROL__GRPC__COMPLETION_QUEUE_COUNT" => "10",
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

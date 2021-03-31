//! Settings for application servers.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use derive_more::{Display, From};
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

/// Public URL of HTTP server. Address for exposed [Client API].
/// It's assumed that HTTP server can be reached via this URL externally.
///
/// This address is returned from [Control API] in `sids` field
/// and [Jason] uses this address to start its session.
///
/// [Client API]: https://tinyurl.com/yx9thsnr
/// [Control API]: https://tinyurl.com/yxsqplq7
/// [Jason]: https://github.com/instrumentisto/medea/tree/master/jason
#[derive(Clone, Debug, Display, Deserialize, Serialize, From)]
pub struct PublicUrl(pub String);

/// [Client API] servers settings.
///
/// [Client API]: https://tinyurl.com/yx9thsnr
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct ClientApiServer {
    /// [Client API] HTTP server settings.
    ///
    /// [Client API]: https://tinyurl.com/yx9thsnr
    pub http: ClientApiHttpServer,
}

/// [Client API] HTTP server settings.
///
/// [Client API]: https://tinyurl.com/yx9thsnr
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct ClientApiHttpServer {
    /// Public URL of HTTP server. Address for exposed [Client API].
    /// It's assumed that HTTP server can be reached via this URL externally.
    ///
    /// This address is returned from [Control API] in `sids` field
    /// and [Jason] uses this address to start its session.
    ///
    /// Defaults to `ws://127.0.0.1:8080/ws`.
    ///
    /// [Client API]: https://tinyurl.com/yx9thsnr
    /// [Control API]: https://tinyurl.com/yxsqplq7
    /// [Jason]: https://github.com/instrumentisto/medea/tree/master/jason
    #[default(PublicUrl("ws://127.0.0.1:8080/ws".to_owned()))]
    pub public_url: PublicUrl,

    /// IP address to bind HTTP server to.
    ///
    /// Defaults to `0.0.0.0`.
    #[default(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)))]
    pub bind_ip: IpAddr,

    /// Port to bind HTTP server to.
    ///
    /// Defaults to `8080`.
    #[default = 8080]
    pub bind_port: u16,
}

impl ClientApiHttpServer {
    /// Builds a [`SocketAddr`] from `bind_ip` and `bind_port`.
    #[inline]
    #[must_use]
    pub fn bind_addr(&self) -> SocketAddr {
        (self.bind_ip, self.bind_port).into()
    }
}

/// [Control API] servers settings.
///
/// [Control API]: https://tinyurl.com/yxsqplq7
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct ControlApiServer {
    /// [Control API] gRPC server settings.
    ///
    /// [Control API]: https://tinyurl.com/yxsqplq7
    pub grpc: ControlApiGrpcServer,
}

/// [Control API] gRPC server settings.
///
/// [Control API]: https://tinyurl.com/yxsqplq7
#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct ControlApiGrpcServer {
    /// IP address to bind gRPC server to.
    ///
    /// Defaults to `0.0.0.0`.
    #[default(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)))]
    pub bind_ip: IpAddr,

    /// Port to bind gRPC server to.
    ///
    /// Defaults to `6565`.
    #[default = 6565]
    pub bind_port: u16,
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
mod client_http_spec {
    use std::net::Ipv4Addr;

    use serial_test::serial;

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
mod control_grpc_spec {
    use std::net::Ipv4Addr;

    use serial_test::serial;

    use crate::{conf::Conf, overrided_by_env_conf};

    #[test]
    #[serial]
    fn overrides_defaults() {
        let default_conf = Conf::default();
        let env_conf = overrided_by_env_conf!(
            "MEDEA_SERVER__CONTROL__GRPC__BIND_IP" => "182.98.12.48",
            "MEDEA_SERVER__CONTROL__GRPC__BIND_PORT" => "44444",
        );

        assert_ne!(
            default_conf.server.control.grpc.bind_ip,
            env_conf.server.control.grpc.bind_ip
        );
        assert_ne!(
            default_conf.server.control.grpc.bind_port,
            env_conf.server.control.grpc.bind_port
        );
        assert_eq!(env_conf.server.control.grpc.bind_port, 44444);
        assert_eq!(
            env_conf.server.control.grpc.bind_ip,
            Ipv4Addr::new(182, 98, 12, 48)
        );
    }
}

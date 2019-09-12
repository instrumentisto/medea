//! Settings for application servers.

use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

use super::{
    client_api_http::ClientApiHttpServer,
    control_api_grpc::ControlApiGrpcServer,
};

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
    pub http: ClientApiHttpServer,
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
    pub grpc: ControlApiGrpcServer,
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

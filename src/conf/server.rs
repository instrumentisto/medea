use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

use super::{
    client_api_http::ClientApiHttpServer,
    control_api_grpc::ControlApiGrpcServer,
};

#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct ClientApiServer {
    /// [Client API] server settings.
    ///
    /// [Client API]: http://tiny.cc/c80uaz
    pub http: ClientApiHttpServer,
}

#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct ControlApiServer {
    /// gRPC [Control API] server settings.
    ///
    /// [Control API]: http://tiny.cc/380uaz
    pub grpc: ControlApiGrpcServer,
}

#[derive(Clone, Debug, Deserialize, Serialize, SmartDefault)]
#[serde(default)]
pub struct Server {
    pub client: ClientApiServer,

    pub control: ControlApiServer,
}

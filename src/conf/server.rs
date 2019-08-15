//! Servers related settings.

use serde::{Deserialize, Serialize};

use super::{grpc::Grpc, http_server::HttpServer};

/// Servers related settings.
#[derive(Clone, Debug, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct Server {
    /// RPC connection settings.
    pub http: HttpServer,

    /// gRPC server settings.
    pub grpc: Grpc,
}

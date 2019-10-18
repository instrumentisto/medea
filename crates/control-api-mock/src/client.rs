//! Implementation of client for Medea's gRPC Control API.

use std::sync::Arc;

use futures::{Future, IntoFuture};
use grpcio::{ChannelBuilder, EnvBuilder, Error};
use medea_control_api_proto::grpc::{
    api::{CreateRequest, CreateResponse, GetResponse, IdRequest, Response},
    api_grpc::ControlApiClient,
};
use protobuf::RepeatedField;

use crate::server::Element;

/// Uri to `Room` element.
#[derive(Clone, Debug)]
pub struct Uri(String);

impl From<String> for Uri {
    fn from(path: String) -> Self {
        Self(format!("{}", path))
    }
}

impl From<(String, String)> for Uri {
    fn from(path: (String, String)) -> Self {
        Self(format!("{}/{}", path.0, path.1))
    }
}

impl From<(String, String, String)> for Uri {
    fn from(path: (String, String, String)) -> Self {
        Self(format!("{}/{}/{}", path.0, path.1, path.2))
    }
}

impl From<()> for Uri {
    fn from(_: ()) -> Self {
        Self(String::new())
    }
}

impl Into<String> for Uri {
    fn into(self) -> String {
        self.0
    }
}

/// Returns new [`IdRequest`] with provided IDs.
fn id_request(ids: Vec<String>) -> IdRequest {
    let mut req = IdRequest::new();
    let ids = RepeatedField::from(ids);
    req.set_fid(ids);
    req
}

/// Client for Medea's Control API.
#[allow(clippy::module_name_repetitions)]
pub struct ControlClient {
    /// [`grpcio`] gRPC client for Medea Control API.
    grpc_client: ControlApiClient,
}

impl ControlClient {
    /// Creates new client for Medea's control API.
    ///
    /// __Note that call of this function don't checks availability of Control
    /// API gRPC server. Availability checks only on sending request to gRPC
    /// server.__
    pub fn new(medea_addr: &str) -> Self {
        Self {
            grpc_client: new_grpcio_control_api_client(medea_addr),
        }
    }

    pub fn create(
        &self,
        uri: Uri,
        element: Element,
    ) -> impl Future<Item = CreateResponse, Error = Error> {
        let mut req = CreateRequest::new();
        req.set_parent_fid(uri.into());
        match element {
            Element::Room(room) => {
                req.set_room(room.into());
            }
            Element::Member(member) => {
                req.set_member(member.into());
            }
            Element::WebRtcPlayEndpoint(webrtc_play) => {
                req.set_webrtc_play(webrtc_play.into());
            }
            Element::WebRtcPublishEndpoint(webrtc_pub) => {
                req.set_webrtc_pub(webrtc_pub.into());
            }
        }

        self.grpc_client
            .create_async(&req)
            .into_future()
            .and_then(|r| r)
    }

    /// Gets single element from Control API by local URI.
    pub fn get(
        &self,
        uri: Uri,
    ) -> impl Future<Item = GetResponse, Error = Error> {
        let req = id_request(vec![uri.into()]);

        self.grpc_client
            .get_async(&req)
            .into_future()
            .and_then(|r| r)
    }

    /// Deletes single element.
    pub fn delete(
        &self,
        uri: Uri,
    ) -> impl Future<Item = Response, Error = Error> {
        let req = id_request(vec![uri.into()]);

        self.grpc_client
            .delete_async(&req)
            .into_future()
            .and_then(|r| r)
    }
}

/// Returns new [`grpcio`] gRPC client for Control API.
fn new_grpcio_control_api_client(addr: &str) -> ControlApiClient {
    let env = Arc::new(EnvBuilder::new().build());
    let ch = ChannelBuilder::new(env).connect(addr);
    ControlApiClient::new(ch)
}

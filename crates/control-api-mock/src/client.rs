//! Implementation of client for Medea's gRPC Control API.

use std::sync::Arc;

use futures::{Future, IntoFuture};
use grpcio::{ChannelBuilder, EnvBuilder, Error};
use medea_control_api_proto::grpc::{
    control_api::{
        CreateRequest, CreateResponse, GetResponse, IdRequest, Response,
    },
    control_api_grpc::ControlApiClient,
};
use protobuf::RepeatedField;

use crate::server::{endpoint::Endpoint, member::Member, room::Room, Element};

/// Uri to `Room` element.
#[derive(Clone, Debug)]
pub struct Uri(String);

impl From<String> for Uri {
    fn from(path: String) -> Self {
        Self(format!("local://{}", path))
    }
}

impl From<(String, String)> for Uri {
    fn from(path: (String, String)) -> Self {
        Self(format!("local://{}/{}", path.0, path.1))
    }
}

impl From<(String, String, String)> for Uri {
    fn from(path: (String, String, String)) -> Self {
        Self(format!("local://{}/{}/{}", path.0, path.1, path.2))
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
    req.set_id(ids);
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
        req.set_id(uri.into());
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

    /// Creates `Room` element with provided [`RoomUri`] and `Room` spec.
    pub fn create_room(
        &self,
        uri: Uri,
        room: Room,
    ) -> impl Future<Item = CreateResponse, Error = Error> {
        let mut req = CreateRequest::new();
        req.set_room(room.into());
        req.set_id(uri.into());

        self.grpc_client
            .create_async(&req)
            .into_future()
            .and_then(|r| r)
    }

    /// Creates `Member` element with provided [`MemberUri`] and `Member` spec.
    pub fn create_member(
        &self,
        uri: Uri,
        member: Member,
    ) -> impl Future<Item = CreateResponse, Error = Error> {
        let mut req = CreateRequest::new();
        req.set_member(member.into());
        req.set_id(uri.into());

        self.grpc_client
            .create_async(&req)
            .into_future()
            .and_then(|r| r)
    }

    /// Creates `Endpoint` element with provided [`EndpointUri`] and `Endpoint`
    /// spec.
    pub fn create_endpoint(
        &self,
        uri: Uri,
        endpoint: Endpoint,
    ) -> impl Future<Item = CreateResponse, Error = Error> {
        let mut req = CreateRequest::new();
        req.set_id(uri.into());
        match endpoint {
            Endpoint::WebRtcPlayEndpoint { spec } => {
                req.set_webrtc_play(spec.into());
            }
            Endpoint::WebRtcPublishEndpoint { spec } => {
                req.set_webrtc_pub(spec.into());
            }
        }

        self.grpc_client
            .create_async(&req)
            .into_future()
            .and_then(|r| r)
    }

    /// Gets single element from Control API by local URI.
    pub fn get_single(
        &self,
        uri: Uri,
    ) -> impl Future<Item = GetResponse, Error = Error> {
        let req = id_request(vec![uri.into()]);

        self.grpc_client
            .get_async(&req)
            .into_future()
            .and_then(|r| r)
    }

    /// Gets all elements with provided Local URIs.
    pub fn get_batch(
        &self,
        uris: Vec<String>,
    ) -> impl Future<Item = GetResponse, Error = Error> {
        let req = id_request(uris);

        self.grpc_client
            .get_async(&req)
            .into_future()
            .and_then(|r| r)
    }

    /// Deletes single element.
    pub fn delete_single(
        &self,
        uri: Uri,
    ) -> impl Future<Item = Response, Error = Error> {
        let req = id_request(vec![uri.into()]);

        self.grpc_client
            .delete_async(&req)
            .into_future()
            .and_then(|r| r)
    }

    /// Deletes all elements with provided local URIs.
    pub fn delete_batch(
        &self,
        ids: Vec<String>,
    ) -> impl Future<Item = Response, Error = Error> {
        let req = id_request(ids);

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

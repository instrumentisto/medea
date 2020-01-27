//! Implementation of client for [Medea]'s gRPC [Control API].
//!
//! [Medea]: https://github.com/instrumentisto/medea
//! [Control API]: https://tinyurl.com/yxsqplq7

use std::sync::Arc;

use futures::{Future, IntoFuture};
use medea_control_api_proto::grpc::medea::{
    control_api_client::ControlApiClient,
    create_request::El as CreateRequestElProto, CreateRequest, CreateResponse,
    GetResponse, IdRequest, Response,
};
use protobuf::RepeatedField;
use slog_scope::debug;
use tonic::{transport::Channel, Status};

use crate::api::Element;

/// Fid to `Room` element.
#[derive(Clone, Debug)]
pub struct Fid(String);

impl From<()> for Fid {
    fn from(_: ()) -> Self {
        Self(String::new())
    }
}

impl From<String> for Fid {
    fn from(path: String) -> Self {
        Self(path)
    }
}

impl From<(String, String)> for Fid {
    fn from(path: (String, String)) -> Self {
        Self(format!("{}/{}", path.0, path.1))
    }
}

impl From<(String, String, String)> for Fid {
    fn from(path: (String, String, String)) -> Self {
        Self(format!("{}/{}/{}", path.0, path.1, path.2))
    }
}

impl Into<String> for Fid {
    fn into(self) -> String {
        self.0
    }
}

/// Returns new [`IdRequest`] with provided FIDs.
fn id_request(ids: Vec<String>) -> IdRequest {
    IdRequest { fid: ids }
}

/// Client for [Medea]'s [Control API].
///
/// [Medea]: https://github.com/instrumentisto/medea
/// [Control API]: https://tinyurl.com/yxsqplq7
pub struct ControlClient {
    medea_addr: String,

    /// [`grpcio`] gRPC client for Medea Control API.
    grpc_client: Option<ControlApiClient<Channel>>,
}

impl ControlClient {
    /// Creates new client for Medea's Control API.
    ///
    /// __Note that call of this function doesn't checks availability of Control
    /// API gRPC server. Availability will be checked only on sending request to
    /// gRPC server.__
    #[must_use]
    pub fn new(medea_addr: String) -> Self {
        Self {
            medea_addr,
            grpc_client: None,
        }
    }

    async fn get_client(&mut self) -> &mut ControlApiClient<Channel> {
        let qq = &mut self.grpc_client;
        if let Some(client) = qq {
            client
        } else {
            let client =
                new_grpcio_control_api_client(self.medea_addr.clone()).await;
            *qq = Some(client);

            qq.as_mut().unwrap()
        }
    }

    /// Creates provided element with gRPC Control API.
    pub async fn create(
        &mut self,
        id: String,
        fid: Fid,
        element: Element,
    ) -> Result<CreateResponse, Status> {
        let el = match element {
            Element::Room(room) => {
                CreateRequestElProto::Room(room.into_proto(id))
            }
            Element::Member(member) => {
                CreateRequestElProto::Member(member.into_proto(id))
            }
            Element::WebRtcPlayEndpoint(webrtc_play) => {
                CreateRequestElProto::WebrtcPlay(webrtc_play.into_proto(id))
            }
            Element::WebRtcPublishEndpoint(webrtc_pub) => {
                CreateRequestElProto::WebrtcPub(webrtc_pub.into_proto(id))
            }
        };
        let req = CreateRequest {
            parent_fid: fid.into(),
            el: Some(el),
        };

        println!("\n\n\n\n\n{:?}\n\n\n\n\n\n", req);

        self.get_client()
            .await
            .create(tonic::Request::new(req))
            .await
            .map(|resp| resp.into_inner())
    }

    /// Gets element from Control API by FID.
    pub async fn get(&mut self, fid: Fid) -> Result<GetResponse, Status> {
        let req = id_request(vec![fid.into()]);

        let resp = self
            .get_client()
            .await
            .get(tonic::Request::new(req))
            .await
            .map(|resp| resp.into_inner());
        debug!("Get response {:?}", resp);
        resp
    }

    /// Deletes element from Control API by FID.
    pub async fn delete(&mut self, fid: Fid) -> Result<Response, Status> {
        let req = id_request(vec![fid.into()]);

        self.get_client()
            .await
            .delete(tonic::Request::new(req))
            .await
            .map(|resp| resp.into_inner())
    }
}

/// Returns new [`grpcio`] gRPC client for Control API.
async fn new_grpcio_control_api_client(
    addr: String,
) -> ControlApiClient<Channel> {
    ControlApiClient::connect(addr).await.unwrap()
}

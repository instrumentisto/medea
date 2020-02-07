//! Implementation of client for [Medea]'s gRPC [Control API].
//!
//! [Medea]: https://github.com/instrumentisto/medea
//! [Control API]: https://tinyurl.com/yxsqplq7

use medea_control_api_proto::grpc::api as proto;
use proto::control_api_client::ControlApiClient;
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

/// Returns new [`proto::IdRequest`] with provided FIDs.
fn id_request(ids: Vec<String>) -> proto::IdRequest {
    proto::IdRequest { fid: ids }
}

/// Client for [Medea]'s [Control API].
///
/// [Medea]: https://github.com/instrumentisto/medea
/// [Control API]: https://tinyurl.com/yxsqplq7
#[derive(Clone)]
pub struct ControlClient {
    /// [`tonic`] gRPC client for Medea Control API.
    grpc_client: ControlApiClient<Channel>,
}

impl ControlClient {
    /// Creates a new client for Medea's Control API.
    ///
    /// __Note that call of this function doesn't checks availability of Control
    /// API gRPC server. Availability will be checked only on sending request to
    /// gRPC server.__
    pub async fn new(
        medea_addr: String,
    ) -> Result<Self, tonic::transport::Error> {
        let client = ControlApiClient::connect(medea_addr).await?;
        Ok(Self {
            grpc_client: client,
        })
    }

    /// Returns [`ControlApiClient`] of this [`ControlClient`].
    fn get_client(&self) -> ControlApiClient<Channel> {
        self.grpc_client.clone()
    }

    /// Creates provided element with gRPC Control API.
    pub async fn create(
        &self,
        id: String,
        fid: Fid,
        element: Element,
    ) -> Result<proto::CreateResponse, Status> {
        use proto::create_request::El::*;

        let el = match element {
            Element::Room(room) => Room(room.into_proto(id)),
            Element::Member(member) => Member(member.into_proto(id)),
            Element::WebRtcPlayEndpoint(webrtc_play) => {
                WebrtcPlay(webrtc_play.into_proto(id))
            }
            Element::WebRtcPublishEndpoint(webrtc_pub) => {
                WebrtcPub(webrtc_pub.into_proto(id))
            }
        };
        let req = proto::CreateRequest {
            parent_fid: fid.into(),
            el: Some(el),
        };

        self.get_client()
            .create(tonic::Request::new(req))
            .await
            .map(tonic::Response::into_inner)
    }

    /// Gets element from Control API by FID.
    pub async fn get(&self, fid: Fid) -> Result<proto::GetResponse, Status> {
        let req = id_request(vec![fid.into()]);

        self.get_client()
            .get(tonic::Request::new(req))
            .await
            .map(tonic::Response::into_inner)
    }

    /// Deletes element from Control API by FID.
    pub async fn delete(&self, fid: Fid) -> Result<proto::Response, Status> {
        let req = id_request(vec![fid.into()]);

        self.get_client()
            .delete(tonic::Request::new(req))
            .await
            .map(tonic::Response::into_inner)
    }
}

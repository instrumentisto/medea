//! `WebRtcPlayEndpoint` [Control API]'s element implementation.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

use std::convert::TryFrom;

use derive_more::{Display, From};
use medea_control_api_proto::grpc::api as proto;
use serde::Deserialize;

use crate::api::control::{
    callback::url::CallbackUrl, refs::SrcUri, TryFromProtobufError,
};

use super::Id as EndpointId;

/// ID of [`WebRtcPlayEndpoint`].
#[derive(Clone, Debug, Deserialize, Display, Eq, Hash, PartialEq, From)]
#[from(forward)]
pub struct WebRtcPlayId(String);

impl std::convert::From<WebRtcPlayId> for EndpointId {
    fn from(id: WebRtcPlayId) -> Self {
        EndpointId::from(id.0)
    }
}

/// Media element which is able to play media data for client via WebRTC.
#[derive(Clone, Deserialize, Debug)]
pub struct WebRtcPlayEndpoint {
    /// Source URI in format `local://{room_id}/{member_id}/{endpoint_id}`.
    pub src: SrcUri,

    /// URL to which `OnStart` Control API callback will be sent.
    pub on_start: Option<CallbackUrl>,

    /// URL to which `OnStop` Control API callback will be sent.
    pub on_stop: Option<CallbackUrl>,

    /// Option to relay all media through a TURN server forcibly.
    #[serde(default)]
    pub force_relay: bool,
}

impl TryFrom<&proto::WebRtcPlayEndpoint> for WebRtcPlayEndpoint {
    type Error = TryFromProtobufError;

    fn try_from(
        value: &proto::WebRtcPlayEndpoint,
    ) -> Result<Self, Self::Error> {
        let on_start = value
            .on_start
            .clone()
            .map(CallbackUrl::try_from)
            .transpose()?;
        let on_stop = value
            .on_stop
            .clone()
            .map(CallbackUrl::try_from)
            .transpose()?;

        Ok(WebRtcPlayEndpoint {
            src: SrcUri::try_from(value.src.clone())?,
            force_relay: value.force_relay,
            on_stop,
            on_start,
        })
    }
}

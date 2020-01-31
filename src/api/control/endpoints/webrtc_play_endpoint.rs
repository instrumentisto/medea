//! `WebRtcPlayEndpoint` [Control API]'s element implementation.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

use std::convert::TryFrom;

use derive_more::{Display, From, Into};
use medea_control_api_proto::grpc::medea as proto;
use serde::Deserialize;

use crate::api::control::{
    callback::url::CallbackUrl, refs::SrcUri, TryFromProtobufError,
};

/// ID of [`WebRtcPlayEndpoint`].
#[derive(
    Clone, Debug, Deserialize, Display, Eq, Hash, PartialEq, From, Into,
)]
pub struct WebRtcPlayId(String);

/// Media element which is able to play media data for client via WebRTC.
#[derive(Clone, Deserialize, Debug)]
pub struct WebRtcPlayEndpoint {
    /// Source URI in format `local://{room_id}/{member_id}/{endpoint_id}`.
    pub src: SrcUri,

    pub on_start: Option<CallbackUrl>,

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
        let on_start = Some(value.on_start.clone())
            .filter(String::is_empty)
            .map(CallbackUrl::try_from)
            .transpose()?;
        let on_stop = Some(value.on_stop.clone())
            .filter(String::is_empty)
            .map(CallbackUrl::try_from)
            .transpose()?;

        if (on_start.is_some() | on_stop.is_some()) && !value.force_relay {
            return Err(TryFromProtobufError::)
        }

        Ok(Self {
            src: SrcUri::try_from(value.src.clone())?,
            force_relay: value.force_relay,
            on_stop,
            on_start,
        })
    }
}

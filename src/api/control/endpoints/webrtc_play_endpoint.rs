//! `WebRtcPlayEndpoint` [Control API]'s element implementation.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

use std::convert::TryFrom;

use derive_more::{Display, From, Into};
use medea_control_api_proto::grpc::api as proto;
use serde::Deserialize;

use crate::api::control::{
    callback::url::CallbackUrl, refs::SrcUri, TryFromProtobufError,
    Unvalidated, Validated, ValidationError,
};

/// ID of [`WebRtcPlayEndpoint`].
#[derive(
    Clone, Debug, Deserialize, Display, Eq, Hash, PartialEq, From, Into,
)]
pub struct WebRtcPlayId(String);

/// Media element which is able to play media data for client via WebRTC.
#[derive(Clone, Deserialize, Debug)]
pub struct WebRtcPlayEndpoint<T> {
    /// Source URI in format `local://{room_id}/{member_id}/{endpoint_id}`.
    pub src: SrcUri,

    /// URL to which `OnStart` Control API callback will be sent.
    pub on_start: Option<CallbackUrl>,

    /// URL to which `OnStop` Control API callback will be sent.
    pub on_stop: Option<CallbackUrl>,

    /// Option to relay all media through a TURN server forcibly.
    #[serde(default)]
    pub force_relay: bool,

    /// Validation state of the [`WebRtcPlayEndpoint`].
    ///
    /// Can be [`Validated`] or [`Unvalidated`].
    ///
    /// [`serde`] will deserialize [`WebRtcPlayEndpoint`] into [`Unvalidated`]
    /// state. Converting from the gRPC's DTOs will cause the same behavior.
    ///
    /// To use [`WebRtcPlayEndpoint`] you should call
    /// [`WebRtcPlayEndpoint::validate`].
    #[serde(skip)]
    _validation_state: T,
}

impl WebRtcPlayEndpoint<Unvalidated> {
    /// Validates this [`WebRtcPlayEndpoint`].
    ///
    /// # Errors
    ///
    /// 1. Returns [`ValidationError::ForceRelayShouldBeEnabled`] if
    ///    [`WebRtcPlayEndpoint::on_start`] or [`WebRtcPlayEndpoint::
    ///    on_stop`] is set, but [`WebRtcPlayEndpoint::force_relay`] is set to
    ///    `false`.
    pub fn validate(
        self,
    ) -> Result<WebRtcPlayEndpoint<Validated>, ValidationError> {
        Ok(WebRtcPlayEndpoint {
            src: self.src,
            on_start: self.on_start,
            on_stop: self.on_stop,
            force_relay: self.force_relay,
            _validation_state: Validated,
        })
    }
}

impl TryFrom<&proto::WebRtcPlayEndpoint> for WebRtcPlayEndpoint<Validated> {
    type Error = TryFromProtobufError;

    fn try_from(
        value: &proto::WebRtcPlayEndpoint,
    ) -> Result<Self, Self::Error> {
        let on_start = Some(value.on_start.clone())
            .filter(|s| !s.is_empty())
            .map(CallbackUrl::try_from)
            .transpose()?;
        let on_stop = Some(value.on_stop.clone())
            .filter(|s| !s.is_empty())
            .map(CallbackUrl::try_from)
            .transpose()?;

        let unvalidated = WebRtcPlayEndpoint {
            src: SrcUri::try_from(value.src.clone())?,
            force_relay: value.force_relay,
            on_stop,
            on_start,
            _validation_state: Unvalidated,
        };

        Ok(unvalidated.validate()?)
    }
}

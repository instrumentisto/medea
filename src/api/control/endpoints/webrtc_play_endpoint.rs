//! `WebRtcPlayEndpoint` [Control API]'s element implementation.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

use std::convert::TryFrom;

use derive_more::{Display, From, Into};
use failure::Fail;
use medea_control_api_proto::grpc::medea as proto;
use serde::{de::Deserializer, Deserialize};

use crate::api::control::{
    callback::url::CallbackUrl, refs::SrcUri, TryFromProtobufError,
};
use std::marker::PhantomData;

/// ID of [`WebRtcPlayEndpoint`].
#[derive(
    Clone, Debug, Deserialize, Display, Eq, Hash, PartialEq, From, Into,
)]
pub struct WebRtcPlayId(String);

#[derive(Debug, Default, Clone)]
pub struct Unvalidated;

#[derive(Debug, Clone)]
pub struct Validated;

#[derive(Debug, Fail, Display)]
pub enum ValidationError {
    ForceRelayShouldBeEnabled,
}

/// Media element which is able to play media data for client via WebRTC.
#[derive(Clone, Deserialize, Debug)]
pub struct WebRtcPlayEndpoint<T> {
    /// Source URI in format `local://{room_id}/{member_id}/{endpoint_id}`.
    pub src: SrcUri,

    pub on_start: Option<CallbackUrl>,

    pub on_stop: Option<CallbackUrl>,

    /// Option to relay all media through a TURN server forcibly.
    pub force_relay: bool,

    #[serde(skip)]
    #[serde(bound = "T: From<Unvalidated> + Default")]
    _validation_state: T,
}

impl WebRtcPlayEndpoint<Unvalidated> {
    pub fn validate(
        self,
    ) -> Result<WebRtcPlayEndpoint<Validated>, ValidationError> {
        if !self.force_relay
            && (self.on_start.is_some() || self.on_stop.is_some())
        {
            Err(ValidationError::ForceRelayShouldBeEnabled)
        } else {
            Ok(WebRtcPlayEndpoint {
                src: self.src,
                on_start: self.on_start,
                on_stop: self.on_stop,
                force_relay: self.force_relay,
                _validation_state: Validated,
            })
        }
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

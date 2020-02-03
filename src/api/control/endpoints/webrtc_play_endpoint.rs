//! `WebRtcPlayEndpoint` [Control API]'s element implementation.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

use std::convert::TryFrom;

use derive_more::{Display, From, Into};
use medea_control_api_proto::grpc::medea as proto;
use serde::{de::Deserializer, Deserialize};

use crate::api::control::{
    callback::url::CallbackUrl, refs::SrcUri, TryFromProtobufError,
};

/// ID of [`WebRtcPlayEndpoint`].
#[derive(
    Clone, Debug, Deserialize, Display, Eq, Hash, PartialEq, From, Into,
)]
pub struct WebRtcPlayId(String);

/// Media element which is able to play media data for client via WebRTC.
#[derive(Clone, Debug)]
pub struct WebRtcPlayEndpoint {
    /// Source URI in format `local://{room_id}/{member_id}/{endpoint_id}`.
    pub src: SrcUri,

    pub on_start: Option<CallbackUrl>,

    pub on_stop: Option<CallbackUrl>,

    /// Option to relay all media through a TURN server forcibly.
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

        if !value.force_relay && (on_start.is_some() || on_stop.is_some()) {
            return Err(
                TryFromProtobufError::CallbackNotSupportedInNotRelayMode,
            );
        }

        Ok(Self {
            src: SrcUri::try_from(value.src.clone())?,
            force_relay: value.force_relay,
            on_stop,
            on_start,
        })
    }
}

impl<'de> Deserialize<'de> for WebRtcPlayEndpoint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error as _;

        let ev = serde_json::Value::deserialize(deserializer)?;
        let map = ev.as_object().ok_or_else(|| {
            D::Error::custom(format!(
                "unable to deserialize ClientMsg [{:?}]",
                &ev
            ))
        })?;

        let src = map
            .get("src")
            .ok_or_else(|| D::Error::custom(format!("missing field `src`")))?;
        let src = SrcUri::deserialize(src).map_err(|e| {
            D::Error::custom(format!(
                "error while deserialization of `src` field: {:?}",
                e
            ))
        })?;
        let force_relay = map
            .get("force_relay")
            .and_then(|force_relay| force_relay.as_bool())
            .unwrap_or_default();

        let on_start = if let Some(on_start) = map.get("on_start") {
            if !force_relay {
                return Err(D::Error::custom(format!(
                    "`on_start` callback not supported while `force_relay` != \
                     `true`"
                )));
            } else {
                Some(CallbackUrl::deserialize(on_start).map_err(|e| {
                    D::Error::custom(format!(
                        "error while deserialization of `on_start` field: {:?}",
                        e
                    ))
                })?)
            }
        } else {
            None
        };

        let on_stop = if let Some(on_stop) = map.get("on_stop") {
            if !force_relay {
                return Err(D::Error::custom(format!(
                    "`on_stop` callback not supported while `force_relay` != \
                     `true`"
                )));
            } else {
                Some(CallbackUrl::deserialize(on_stop).map_err(|e| {
                    D::Error::custom(format!(
                        "error while deserialization of `on_stop` field: {:?}",
                        e
                    ))
                })?)
            }
        } else {
            None
        };

        Ok(Self {
            src,
            force_relay,
            on_start,
            on_stop,
        })
    }
}

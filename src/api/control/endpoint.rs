//! Definitions and implementations of [Control API]'s `Endpoint`s elements.
//!
//! [Control API]: http://tiny.cc/380uaz

use std::{convert::TryFrom, fmt, str::FromStr};

use serde::{
    de::{self, Deserializer, Error, Visitor},
    Deserialize,
};
use url::Url;

use crate::api::control::MemberId;

use super::{member::MemberElement, TryFromElementError};

/// Media element that one or more media data streams flow through.
#[derive(Debug)]
pub enum Endpoint {
    WebRtcPublish(WebRtcPublishEndpoint),
    WebRtcPlay(WebRtcPlayEndpoint),
}

/// Possible schemes of media elements URIs.
#[derive(Clone, Debug)]
pub enum Scheme {
    /// `local://` scheme which refers to a local in-memory media element.
    Local,
}

impl FromStr for Scheme {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "local" => Ok(Scheme::Local),
            _ => Err(format!("cannot parse \"{}\" to Scheme", s)),
        }
    }
}

impl TryFrom<&MemberElement> for Endpoint {
    type Error = TryFromElementError;

    fn try_from(from: &MemberElement) -> Result<Self, Self::Error> {
        match from {
            MemberElement::WebRtcPlayEndpoint { spec } => {
                Ok(Self::WebRtcPlay(spec.clone()))
            }
            MemberElement::WebRtcPublishEndpoint { spec } => {
                Ok(Self::WebRtcPublish(spec.clone()))
            }
        }
    }
}

/// Peer-to-peer mode of [`WebRtcPublishEndpoint`].
#[derive(Clone, Deserialize, Debug)]
pub enum P2pMode {
    /// Always connect peer-to-peer.
    Always,
}

/// Media element which is able to publish media data for another client via
/// WebRTC.
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Deserialize, Debug)]
pub struct WebRtcPublishEndpoint {
    /// Peer-to-peer mode.
    pub p2p: P2pMode,
}

/// Media element which is able to play media data for client via WebRTC.
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Deserialize, Debug)]
pub struct WebRtcPlayEndpoint {
    /// Source URI in format `local://{room_id}/{member_id}/{endpoint_id}`.
    pub src: SrcUri,
}

/// Special uri with pattern `local://{room_id}/{member_id}/{endpoint_id}`.
#[derive(Clone, Debug)]
pub struct SrcUri {
    /// Scheme of media element URI.
    pub scheme: Scheme,

    /// ID of [`Room`]
    ///
    /// [`Room`]: crate::signalling::room::Room
    pub room_id: String,

    /// ID of `Member`
    pub member_id: MemberId,

    /// Control ID of [`Endpoint`]
    pub endpoint_id: String,
}

/// Deserialization for [`SrcUri`] with pattern
/// `local://{room_id}/{member_id}/{endpoint_id}`.
impl<'de> Deserialize<'de> for SrcUri {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SrcUriVisitor;

        impl<'de> Visitor<'de> for SrcUriVisitor {
            type Value = SrcUri;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str(
                    "URI in format local://room_id/member_id/endpoint_id",
                )
            }

            fn visit_str<E>(self, value: &str) -> Result<SrcUri, E>
            where
                E: de::Error,
            {
                let uri = Url::parse(value).map_err(|_| {
                    Error::custom(format!("'{}' is not URI", value))
                })?;
                let scheme = FromStr::from_str(uri.scheme()).map_err(|_| {
                    Error::custom(format!(
                        "cannot parse URI scheme '{}'",
                        value
                    ))
                })?;
                let room_id = uri
                    .host()
                    .ok_or_else(|| {
                        Error::custom(format!(
                            "cannot parse room ID from URI '{}'",
                            value
                        ))
                    })?
                    .to_string();
                let mut path = uri.path_segments().ok_or_else(|| {
                    Error::custom(format!(
                        "cannot parse member and endpoint IDs from URI '{}'",
                        value
                    ))
                })?;
                let member_id = path
                    .next()
                    .map(|id| MemberId(id.to_owned()))
                    .ok_or_else(|| {
                    Error::custom(format!(
                        "cannot parse member ID from URI '{}'",
                        value
                    ))
                })?;
                let endpoint_id =
                    path.next().map(ToOwned::to_owned).ok_or_else(|| {
                        Error::custom(format!(
                            "cannot parse endpoint ID from URI '{}'",
                            value
                        ))
                    })?;
                Ok(SrcUri {
                    scheme,
                    room_id,
                    member_id,
                    endpoint_id,
                })
            }
        }

        deserializer.deserialize_identifier(SrcUriVisitor)
    }
}

#[cfg(test)]
mod src_uri_deserialization_tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Deserialize)]
    struct SrcUriTest {
        src: SrcUri,
    }

    #[test]
    fn deserializes() {
        let uri: SrcUriTest = serde_json::from_str(
            r#"{ "src": "local://room_id/member_id/endpoint_id" }"#,
        )
        .unwrap();

        assert_eq!(uri.src.member_id, MemberId("member_id".into()));
        assert_eq!(uri.src.room_id, "room_id".to_string());
        assert_eq!(uri.src.endpoint_id, "endpoint_id".to_string());
    }

    #[test]
    fn errors_on_incorrect_scheme() {
        let res = serde_json::from_str::<SrcUriTest>(
            r#"{ "src": "not_local://room_id/member_id/endpoint_id" }"#,
        );

        assert!(res.is_err())
    }

    #[test]
    fn errors_when_endpoint_is_absent() {
        let res = serde_json::from_str::<SrcUriTest>(
            r#"{ "src": "local://room_id/member_id" }"#,
        );

        assert!(res.is_err())
    }
}

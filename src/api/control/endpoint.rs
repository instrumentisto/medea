//! Control API specification Endpoint definitions.

use std::{convert::TryFrom, fmt, str::FromStr};

use serde::{
    de::{self, Deserializer, Error, Visitor},
    Deserialize,
};
use url::Url;

use crate::api::control::MemberId;

use super::{member::MemberElement, TryFromElementError};

/// [`Endpoint`] represents a media element that one or more media data streams
/// flow through.
#[derive(Debug)]
pub enum Endpoint {
    WebRtcPublish(WebRtcPublishEndpoint),
    WebRtcPlay(WebRtcPlayEndpoint),
}

#[derive(Clone, Debug)]
pub enum Scheme {
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

/// Serde deserializer for [`SrcUri`].
/// Deserialize URIs with pattern `local://{room_id}/{member_id}/{endpoint_id}`.
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
                    "Uri in format local://room_id/member_id/endpoint_id",
                )
            }

            fn visit_str<E>(self, value: &str) -> Result<SrcUri, E>
            where
                E: de::Error,
            {
                let uri = Url::parse(value).map_err(|_| {
                    Error::custom(format!("'{}' is not URL", value))
                })?;

                let scheme = match FromStr::from_str(uri.scheme()) {
                    Ok(scheme) => scheme,
                    Err(_) => {
                        return Err(Error::custom(format!(
                            "cannot parse uri scheme \"{}\"",
                            value
                        )))
                    }
                };
                let room_id = match uri.host() {
                    Some(host) => host.to_string(),
                    None => {
                        return Err(Error::custom(format!(
                            "cannot parse uri scheme \"{}\"",
                            value
                        )))
                    }
                };

                let mut path = match uri.path_segments() {
                    Some(path) => path,
                    None => {
                        return Err(Error::custom(format!(
                            "cannot parse uri segments \"{}\"",
                            value
                        )))
                    }
                };

                let member_id = match path.next() {
                    Some(member_id) => MemberId(member_id.to_owned()),
                    None => {
                        return Err(Error::custom(format!(
                            "cannot parse member_id \"{}\"",
                            value
                        )))
                    }
                };

                let endpoint_id = match path.next() {
                    Some(endpoint_id) => endpoint_id.to_owned(),
                    None => {
                        return Err(Error::custom(format!(
                            "cannot parse endpoint_id \"{}\"",
                            value
                        )))
                    }
                };

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
    fn deserialize() {
        let valid_json_uri =
            r#"{ "src": "local://room_id/member_id/endpoint_id" }"#;
        let local_uri: SrcUriTest =
            serde_json::from_str(valid_json_uri).unwrap();

        assert_eq!(
            local_uri.src.member_id,
            MemberId(String::from("member_id"))
        );
        assert_eq!(local_uri.src.room_id, String::from("room_id"));
        assert_eq!(local_uri.src.endpoint_id, String::from("endpoint_id"));
    }

    #[test]
    fn return_error_when_uri_not_local() {
        let invalid_json_uri =
            r#"{ "src": "not_local://room_id/member_id/endpoint_id" }"#;
        if serde_json::from_str::<SrcUriTest>(invalid_json_uri).is_ok() {
            unreachable!()
        }
    }

    #[test]
    fn return_error_when_uri_is_not_full() {
        let invalid_json_uri = r#"{ "src": "local://room_id/member_id" }"#;
        if serde_json::from_str::<SrcUriTest>(invalid_json_uri).is_ok() {
            unreachable!()
        }
    }
}

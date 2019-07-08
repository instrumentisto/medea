//! Control API specification Endpoint definitions.

use std::{convert::TryFrom, fmt};

use failure::Fail;
use macro_attr::*;
use newtype_derive::{newtype_fmt, NewtypeDisplay, NewtypeFrom};
use serde::{
    de::{self, Deserializer, Error, Visitor},
    Deserialize,
};

use crate::api::control::grpc::protos::control::{
    Member_Element as MemberElementProto,
    WebRtcPlayEndpoint as WebRtcPlayEndpointProto,
    WebRtcPublishEndpoint as WebRtcPublishEndpointProto,
    WebRtcPublishEndpoint_P2P as WebRtcPublishEndpointP2pProto,
};

use super::{
    local_uri::{LocalUri, LocalUriParseError},
    Element, MemberId, RoomId, TryFromElementError, TryFromProtobufError,
};

macro_attr! {
    /// ID of [`Room`].
    #[derive(
        Clone,
        Debug,
        Deserialize,
        Eq,
        Hash,
        PartialEq,
        NewtypeFrom!,
        NewtypeDisplay!,
    )]
    pub struct WebRtcPublishId(pub String);
}

macro_attr! {
    /// ID of [`Room`].
    #[derive(
        Clone,
        Debug,
        Deserialize,
        Eq,
        Hash,
        PartialEq,
        NewtypeFrom!,
        NewtypeDisplay!,
    )]
    pub struct WebRtcPlayId(pub String);
}

/// Peer-to-peer mode of [`WebRtcPublishEndpoint`].
#[derive(Clone, Deserialize, Debug)]
pub enum P2pMode {
    /// Always connect peer-to-peer.
    Always,
    Never,
    IfPossible,
}

// TODO: use From
impl TryFrom<WebRtcPublishEndpointP2pProto> for P2pMode {
    type Error = TryFromProtobufError;

    fn try_from(
        value: WebRtcPublishEndpointP2pProto,
    ) -> Result<Self, Self::Error> {
        Ok(match value {
            WebRtcPublishEndpointP2pProto::ALWAYS => P2pMode::Always,
            WebRtcPublishEndpointP2pProto::IF_POSSIBLE => P2pMode::IfPossible,
            WebRtcPublishEndpointP2pProto::NEVER => P2pMode::Never,
        })
    }
}

/// [`Endpoint`] represents a media element that one or more media data streams
/// flow through.
#[derive(Debug)]
pub enum Endpoint {
    WebRtcPublish(WebRtcPublishEndpoint),
    WebRtcPlay(WebRtcPlayEndpoint),
}

impl Into<Element> for Endpoint {
    fn into(self) -> Element {
        match self {
            Endpoint::WebRtcPublish(e) => {
                Element::WebRtcPublishEndpoint { spec: e }
            }
            Endpoint::WebRtcPlay(e) => Element::WebRtcPlayEndpoint { spec: e },
        }
    }
}

impl TryFrom<&MemberElementProto> for Endpoint {
    type Error = TryFromProtobufError;

    fn try_from(value: &MemberElementProto) -> Result<Self, Self::Error> {
        if value.has_webrtc_play() {
            let play = WebRtcPlayEndpoint::try_from(value.get_webrtc_play())?;
            return Ok(Endpoint::WebRtcPlay(play));
        } else if value.has_webrtc_pub() {
            let publish =
                WebRtcPublishEndpoint::try_from(value.get_webrtc_pub())?;
            return Ok(Endpoint::WebRtcPublish(publish));
        } else {
            // TODO
            unimplemented!()
        }
    }
}

impl TryFrom<&Element> for Endpoint {
    type Error = TryFromElementError;

    fn try_from(from: &Element) -> Result<Self, Self::Error> {
        match from {
            Element::WebRtcPlayEndpoint { spec } => {
                Ok(Endpoint::WebRtcPlay(spec.clone()))
            }
            Element::WebRtcPublishEndpoint { spec } => {
                Ok(Endpoint::WebRtcPublish(spec.clone()))
            }
            _ => Err(TryFromElementError::NotEndpoint),
        }
    }
}

/// Media element which is able to publish media data for another client via
/// WebRTC.
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Deserialize, Debug)]
pub struct WebRtcPublishEndpoint {
    /// Peer-to-peer mode.
    pub p2p: P2pMode,
}

impl TryFrom<&WebRtcPublishEndpointProto> for WebRtcPublishEndpoint {
    type Error = TryFromProtobufError;

    fn try_from(
        value: &WebRtcPublishEndpointProto,
    ) -> Result<Self, Self::Error> {
        if value.has_p2p() {
            Ok(Self {
                p2p: P2pMode::try_from(value.get_p2p())?,
            })
        } else {
            Err(TryFromProtobufError::P2pModeNotFound)
        }
    }
}

/// Media element which is able to play media data for client via WebRTC.
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Deserialize, Debug)]
pub struct WebRtcPlayEndpoint {
    /// Source URI in format `local://{room_id}/{member_id}/{endpoint_id}`.
    pub src: SrcUri,
}

impl TryFrom<&WebRtcPlayEndpointProto> for WebRtcPlayEndpoint {
    type Error = TryFromProtobufError;

    fn try_from(value: &WebRtcPlayEndpointProto) -> Result<Self, Self::Error> {
        if value.has_src() {
            Ok(Self {
                src: SrcUri::parse(value.get_src())?,
            })
        } else {
            Err(TryFromProtobufError::SrcUriNotFound)
        }
    }
}

/// Special uri with pattern `local://{room_id}/{member_id}/{endpoint_id}`.
#[derive(Clone, Debug)]
pub struct SrcUri {
    /// ID of [`Room`]
    pub room_id: RoomId,
    /// ID of `Member`
    pub member_id: MemberId,
    /// Control ID of [`Endpoint`]
    pub endpoint_id: WebRtcPublishId,
}

// TODO
#[derive(Debug, Fail)]
pub enum SrcParseError {
    #[fail(display = "Missing fields {:?} in '{}' local URI.", _1, _0)]
    MissingField(String, Vec<String>),
    #[fail(display = "Local URI '{}' parse error: {:?}", _0, _1)]
    LocalUriParseError(String, LocalUriParseError),
}

impl SrcUri {
    pub fn parse(value: &str) -> Result<Self, SrcParseError> {
        let local_uri = LocalUri::parse(value).map_err(|e| {
            SrcParseError::LocalUriParseError(value.to_string(), e)
        })?;

        let mut missing_fields = Vec::new();
        if local_uri.room_id.is_none() {
            missing_fields.push("room_id".to_string());
        }
        if local_uri.member_id.is_none() {
            missing_fields.push("member_id".to_string());
        }
        if local_uri.endpoint_id.is_none() {
            missing_fields.push("endpoint_id".to_string());
        }

        if !missing_fields.is_empty() {
            return Err(SrcParseError::MissingField(
                value.to_string(),
                missing_fields,
            ));
        } else {
            Ok(Self {
                room_id: local_uri.room_id.unwrap(),
                member_id: local_uri.member_id.unwrap(),
                endpoint_id: WebRtcPublishId(local_uri.endpoint_id.unwrap()),
            })
        }
    }
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
                match SrcUri::parse(value) {
                    Ok(src_uri) => Ok(src_uri),
                    Err(e) => Err(Error::custom(e)),
                }
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

    #[inline]
    fn id<T: From<String>>(s: &str) -> T {
        T::from(s.to_string())
    }

    #[test]
    fn deserialize() {
        let valid_json_uri =
            r#"{ "src": "local://room_id/member_id/endpoint_id" }"#;
        let local_uri: SrcUriTest =
            serde_json::from_str(valid_json_uri).unwrap();

        assert_eq!(local_uri.src.member_id, id("member_id"));
        assert_eq!(local_uri.src.room_id, id("room_id"));
        assert_eq!(local_uri.src.endpoint_id, id("endpoint_id"));
    }

    #[test]
    fn return_error_when_uri_not_local() {
        let invalid_json_uri =
            r#"{ "src": "not_local://room_id/member_id/endpoint_id" }"#;
        match serde_json::from_str::<SrcUriTest>(invalid_json_uri) {
            Ok(_) => assert!(false),
            Err(_) => assert!(true),
        }
    }

    #[test]
    fn return_error_when_uri_is_not_full() {
        let invalid_json_uri = r#"{ "src": "local://room_id/member_id" }"#;
        match serde_json::from_str::<SrcUriTest>(invalid_json_uri) {
            Ok(_) => assert!(false),
            Err(_) => assert!(true),
        }
    }

    #[test]
    fn return_error_when_uri_have_empty_part() {
        let invalid_json_uri = r#"{ "src": "local://room_id//endpoint_id" }"#;
        match serde_json::from_str::<SrcUriTest>(invalid_json_uri) {
            Ok(_) => assert!(false),
            Err(_) => assert!(true),
        }
    }
}

//! `WebRtcPlayEndpoint` implementation.

use std::{convert::TryFrom, fmt};

use failure::Fail;
use macro_attr::*;
use newtype_derive::{newtype_fmt, NewtypeDisplay, NewtypeFrom};
use serde::{
    de::{self, Deserializer, Error, Visitor},
    Deserialize,
};

use crate::api::{
    control::{
        endpoints::webrtc_publish_endpoint::WebRtcPublishId,
        grpc::protos::control::WebRtcPlayEndpoint as WebRtcPlayEndpointProto,
        local_uri::{IsEndpointId, LocalUri, LocalUriParseError, LocalUriType},
        MemberId, RoomId, TryFromProtobufError,
    },
    error_codes::ErrorCode,
};

macro_attr! {
    /// ID of [`WebRtcPlayEndpoint`].
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
        Ok(Self {
            src: SrcUri::parse(value.get_src())?,
        })
    }
}

#[derive(Debug, Fail)]
pub enum SrcParseError {
    #[fail(display = "Provided not src uri {}", _0)]
    NotSrcUri(String),
    #[fail(display = "Local URI '{}' parse error: {:?}", _0, _1)]
    LocalUriParseError(String, LocalUriParseError),
}

impl Into<ErrorCode> for SrcParseError {
    fn into(self) -> ErrorCode {
        match self {
            SrcParseError::NotSrcUri(text) => ErrorCode::NotSourceUri(text),
            SrcParseError::LocalUriParseError(_, err) => err.into(),
        }
    }
}

/// Special uri with pattern `local://{room_id}/{member_id}/{endpoint_id}`.
/// This uri can pointing only to [`WebRtcPublishEndpoint`].
#[derive(Clone, Debug)]
pub struct SrcUri {
    /// ID of [`Room`]
    pub room_id: RoomId,
    /// ID of `Member`
    pub member_id: MemberId,
    /// Control ID of [`Endpoint`]
    pub endpoint_id: WebRtcPublishId,
}

impl SrcUri {
    /// Parse [`SrcUri`] from str.
    ///
    /// Returns [`SrcParseError::LocalUriParseError`] when some error happened
    /// while parsing URI.
    pub fn parse(value: &str) -> Result<Self, SrcParseError> {
        let local_uri = LocalUriType::parse(value).map_err(|e| {
            SrcParseError::LocalUriParseError(value.to_string(), e)
        })?;

        if let LocalUriType::Endpoint(endpoint_uri) = local_uri {
            Ok(endpoint_uri.into())
        } else {
            Err(SrcParseError::NotSrcUri(value.to_string()))
        }
    }
}

impl From<LocalUri<IsEndpointId>> for SrcUri {
    fn from(uri: LocalUri<IsEndpointId>) -> Self {
        let (endpoint_id, member_uri) = uri.take_endpoint_id();
        let (member_id, room_uri) = member_uri.take_member_id();
        let room_id = room_uri.take_room_id();

        Self {
            room_id,
            member_id,
            endpoint_id: WebRtcPublishId(endpoint_id),
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

impl fmt::Display for SrcUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "local://{}/{}/{}",
            self.room_id, self.member_id, self.endpoint_id
        )
    }
}

use std::{convert::TryFrom, fmt};

use failure::Fail;
use macro_attr::*;
use newtype_derive::{newtype_fmt, NewtypeDisplay, NewtypeFrom};
use serde::{
    de::{self, Deserializer, Error, Visitor},
    Deserialize,
};

use crate::api::control::{
    endpoints::webrtc_publish_endpoint::WebRtcPublishId,
    grpc::protos::control::WebRtcPlayEndpoint as WebRtcPlayEndpointProto,
    local_uri::{LocalUri, LocalUriParseError},
    MemberId, RoomId, TryFromProtobufError,
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
        if value.has_src() {
            Ok(Self {
                src: SrcUri::parse(value.get_src())?,
            })
        } else {
            Err(TryFromProtobufError::SrcUriNotFound)
        }
    }
}

// TODO
#[derive(Debug, Fail)]
pub enum SrcParseError {
    #[fail(display = "Missing fields {:?} in '{}' local URI.", _1, _0)]
    MissingField(String, Vec<String>),
    #[fail(display = "Local URI '{}' parse error: {:?}", _0, _1)]
    LocalUriParseError(String, LocalUriParseError),
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

impl fmt::Display for SrcUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "local://{}/{}/{}",
            self.room_id, self.member_id, self.endpoint_id
        )
    }
}

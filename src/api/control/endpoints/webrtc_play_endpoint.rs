//! `WebRtcPlayEndpoint` [Control API]'s element implementation.
//!
//! [Control API]: http://tiny.cc/380uaz

use std::{convert::TryFrom, fmt};

use derive_more::{Display, From};
use failure::Fail;
use medea_grpc_proto::control::WebRtcPlayEndpoint as WebRtcPlayEndpointProto;
use serde::{
    de::{self, Deserializer, Error, Visitor},
    Deserialize,
};

use crate::api::control::{
    endpoints::webrtc_publish_endpoint::WebRtcPublishId,
    local_uri::{IsEndpointId, LocalUri, LocalUriParseError, StatefulLocalUri},
    MemberId, RoomId, TryFromProtobufError,
};

/// ID of [`WebRtcPlayEndpoint`].
#[derive(Clone, Debug, Deserialize, Display, Eq, Hash, PartialEq, From)]
pub struct WebRtcPlayId(pub String);

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
            src: SrcUri::try_from(value.get_src())?,
        })
    }
}

/// Errors which can happen while parsing [`SrcUri`] from [Control API] specs.
///
/// [Control API]: http://tiny.cc/380uaz
#[derive(Debug, Fail, Display)]
pub enum SrcParseError {
    /// Provided not source URI.
    #[display(fmt = "Provided not src uri {}", _0)]
    NotSrcUri(String),

    /// Error from [`LocalUri`] parser. This is general errors for [`SrcUri`]
    /// parsing because [`SrcUri`] parses with [`LocalUri`] parser.
    #[display(fmt = "Local URI '{}' parse error: {:?}", _0, _1)]
    LocalUriParseError(String, LocalUriParseError),
}

/// Special uri with pattern `local://{room_id}/{member_id}/{endpoint_id}`.
/// This uri can pointing only to [`WebRtcPublishEndpoint`].
///
/// Note that [`SrcUri`] is parsing with [`LocalUri`] parser.
/// Actually difference between [`SrcUri`] and [`LocalUri`]
/// in endpoint ID's type. In [`SrcUri`] it [`WebRtcPublishId`], and in
/// [`LocalUri`] it [`String`]. Also [`SrcUri`] can be deserialized with
/// [`serde`].
///
/// Atm used only in [Control API] specs.
///
/// [`WebRtcPublishEndpoint`]:
/// crate::api::control::endpoints::WebRtcPublishEndpoint
/// [Control API]: http://tiny.cc/380uaz
#[derive(Clone, Debug)]
pub struct SrcUri {
    /// ID of [`Room`].
    ///
    /// [`Room`]: crate::signalling::room::Room
    pub room_id: RoomId,

    /// ID of [`MemberSpec`].
    ///
    /// [`MemberSpec`]: crate::api::control::member::MemberSpec
    pub member_id: MemberId,

    /// ID of [`WebRtcPublishEndpoint`].
    pub endpoint_id: WebRtcPublishId,
}

impl TryFrom<&str> for SrcUri {
    type Error = SrcParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let local_uri = StatefulLocalUri::try_from(value).map_err(|e| {
            SrcParseError::LocalUriParseError(value.to_string(), e)
        })?;

        if let StatefulLocalUri::Endpoint(endpoint_uri) = local_uri {
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

/// [Serde] deserializer for [`SrcUri`].
/// Deserializes URIs with pattern:
/// `local://room_id/member_id/publish_endpoint_id`.
///
/// [Serde]: serde
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
                match SrcUri::try_from(value) {
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

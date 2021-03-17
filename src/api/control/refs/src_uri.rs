//! Implementation of special URI with pattern
//! `local://{room_id}/{member_id}/{endpoint_id}`. This URI can point only to
//! [`WebRtcPublishEndpoint`].
//!
//! [`WebRtcPublishEndpoint`]:
//! crate::signalling::elements::endpoints::webrtc::WebRtcPublishEndpoint

use std::{convert::TryFrom, fmt};

use derive_more::Display;
use failure::Fail;
use medea_client_api_proto::{MemberId, RoomId};
use serde::{
    de::{self, Deserializer, Error, Visitor},
    Deserialize,
};

use crate::api::control::{
    endpoints::webrtc_publish_endpoint::WebRtcPublishId,
    refs::{
        local_uri::{LocalUriParseError, StatefulLocalUri},
        LocalUri, ToEndpoint,
    },
};

/// Errors which can happen while parsing [`SrcUri`] from [Control API] specs.
///
/// [Control API]: https://tinyurl.com/yxsqplq7
#[derive(Debug, Fail, Display)]
pub enum SrcParseError {
    /// Provided not source URI.
    #[display(fmt = "Provided not src uri {}", _0)]
    NotSrcUri(String),

    /// Error from [`LocalUri`] parser. This is general errors for [`SrcUri`]
    /// parsing because [`SrcUri`] parses with [`LocalUri`] parser.
    #[display(fmt = "Local URI parse error: {:?}", _0)]
    LocalUriParseError(LocalUriParseError),
}

/// Special URI with pattern `local://{room_id}/{member_id}/{endpoint_id}`.
/// This uri can pointing only to [`WebRtcPublishEndpoint`].
///
/// Note that [`SrcUri`] is parsing with [`LocalUri`] parser.
/// Actually difference between [`SrcUri`] and [`LocalUri`]
/// in endpoint ID's type. In [`SrcUri`] it [`WebRtcPublishId`], and in
/// [`LocalUri`] it [`EndpointId`]. Also [`SrcUri`] can be deserialized with
/// [`serde`].
///
/// Atm used only in [Control API] specs.
///
/// [`WebRtcPublishEndpoint`]:
/// crate::api::control::endpoints::WebRtcPublishEndpoint
/// [Control API]: https://tinyurl.com/yxsqplq7
/// [`EndpointId`]: crate::api::control::EndpointId
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
    ///
    /// [`WebRtcPublishEndpoint`]:
    /// crate::api::control::endpoints::WebRtcPublishEndpoint
    pub endpoint_id: WebRtcPublishId,
}

impl TryFrom<String> for SrcUri {
    type Error = SrcParseError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let local_uri = StatefulLocalUri::try_from(value)
            .map_err(SrcParseError::LocalUriParseError)?;

        match local_uri {
            StatefulLocalUri::Room(uri) => {
                Err(SrcParseError::NotSrcUri(uri.to_string()))
            }
            StatefulLocalUri::Member(uri) => {
                Err(SrcParseError::NotSrcUri(uri.to_string()))
            }
            StatefulLocalUri::Endpoint(uri) => Ok(uri.into()),
        }
    }
}

impl From<LocalUri<ToEndpoint>> for SrcUri {
    fn from(uri: LocalUri<ToEndpoint>) -> Self {
        let (room_id, member_id, endpoint_id) = uri.take_all();

        Self {
            room_id,
            member_id,
            endpoint_id: endpoint_id.into(),
        }
    }
}

/// [Serde] deserializer for [`SrcUri`].
///
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
                match SrcUri::try_from(value.to_owned()) {
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

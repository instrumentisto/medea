//! URI for pointing to some Medea element in spec.

// Fix clippy's wrong errors for `Self` in `LocalUri`s with states as generics.
#![allow(clippy::use_self)]

use std::{convert::TryFrom, fmt, string::ToString};

use derive_more::{Display, From};
use failure::Fail;
use medea_client_api_proto::{MemberId, RoomId};
use url::Url;

use super::{SrcUri, ToEndpoint, ToMember, ToRoom};

/// URI in format `local://room_id/member_id/endpoint_id`.
///
/// This kind of URI used for pointing to some element in spec ([`Room`],
/// [`Member`], [`WebRtcPlayEndpoint`], [`WebRtcPublishEndpoint`], etc) based on
/// state.
///
/// [`LocalUri`] can be in three states: [`ToRoom`], [`ToMember`],
/// [`ToRoom`]. This is used for compile time guarantees that some
/// [`LocalUri`] have all mandatory fields.
///
/// You also can take value from [`LocalUri`] without clone, but you have to do
/// it consistently. For example, if you wish to get [`RoomId`], [`MemberId`]
/// and [`Endpoint`] ID from [`LocalUri`] in [`ToEndpoint`] state you should
/// make this steps:
///
/// ```
/// # use medea::api::control::refs::{LocalUri, ToEndpoint};
/// # use medea::api::control::{RoomId, MemberId, EndpointId};
/// #
/// let orig_room_id = RoomId::from("room");
/// let orig_member_id = MemberId::from("member");
/// let orig_endpoint_id = EndpointId::from("endpoint");
///
/// // Create new LocalUri for endpoint.
/// let local_uri = LocalUri::<ToEndpoint>::new(
///     orig_room_id.clone(),
///     orig_member_id.clone(),
///     orig_endpoint_id.clone()
/// );
/// let local_uri_clone = local_uri.clone();
///
/// // We can get reference to room_id from this LocalUri
/// // without taking ownership:
/// assert_eq!(local_uri.room_id(), &orig_room_id);
///
/// // If you want to take all IDs ownership, you should do this steps:
/// let (endpoint_id, member_uri) = local_uri.take_endpoint_id();
/// assert_eq!(endpoint_id, orig_endpoint_id);
///
/// let (member_id, room_uri) = member_uri.take_member_id();
/// assert_eq!(member_id, orig_member_id);
///
/// let room_id = room_uri.take_room_id();
/// assert_eq!(room_id, orig_room_id);
///
/// // Or simply
/// let (room_id, member_id, endpoint_id) = local_uri_clone.take_all();
/// ```
///
/// This is necessary so that it is not possible to get the address in the
/// wrong state (`local://room_id//endpoint_id` for example).
///
/// [`Member`]: crate::signalling::elements::Member
/// [`Room`]: crate::signalling::room::Room
/// [`WebRtcPlayEndpoint`]:
/// crate::signalling::elements::endpoints::webrtc::WebRtcPlayEndpoint
/// [`WebRtcPublishEndpoint`]:
/// crate::signalling::elements::endpoints::webrtc::WebRtcPublishEndpoint
/// [`Endpoint`]: crate::signalling::elements::endpoints::Endpoint
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct LocalUri<T> {
    state: T,
}

impls_for_stateful_refs!(LocalUri);

impl From<StatefulLocalUri> for LocalUri<ToRoom> {
    fn from(from: StatefulLocalUri) -> Self {
        match from {
            StatefulLocalUri::Room(uri) => uri,
            StatefulLocalUri::Member(uri) => {
                let (_, uri) = uri.take_member_id();
                uri
            }
            StatefulLocalUri::Endpoint(uri) => {
                let (_, uri) = uri.take_endpoint_id();
                let (_, uri) = uri.take_member_id();
                uri
            }
        }
    }
}

impl From<SrcUri> for LocalUri<ToEndpoint> {
    fn from(uri: SrcUri) -> Self {
        LocalUri::<ToEndpoint>::new(
            uri.room_id,
            uri.member_id,
            uri.endpoint_id.into(),
        )
    }
}

impl fmt::Display for LocalUri<ToRoom> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "local://{}", self.state.0)
    }
}

impl fmt::Display for LocalUri<ToMember> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "local://{}/{}", self.state.0, self.state.1)
    }
}

impl fmt::Display for LocalUri<ToEndpoint> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "local://{}/{}/{}",
            self.state.0, self.state.1, self.state.2
        )
    }
}

/// Error which can happen while [`LocalUri`] parsing.
#[derive(Debug, Fail, Display)]
pub enum LocalUriParseError {
    /// Protocol of provided URI is not "local://".
    #[display(fmt = "Provided URIs protocol is not 'local://'.")]
    NotLocal(String),

    /// Too many paths in provided URI.
    ///
    /// `local://room_id/member_id/endpoint_id/redundant_path` for example.
    #[display(fmt = "Too many paths in provided URI ({}).", _0)]
    TooManyPaths(String),

    /// Some paths is missing in URI.
    ///
    /// `local://room_id//qwerty` for example.
    #[display(fmt = "Missing fields. {}", _0)]
    MissingPaths(String),

    /// Error while parsing URI by [`url::Url`].
    #[display(fmt = "Error while parsing URL. {:?}", _0)]
    UrlParseErr(String, url::ParseError),

    /// Provided empty URI.
    #[display(fmt = "You provided empty local uri.")]
    Empty,
}

/// Enum for storing [`LocalUri`]s in all states.
#[derive(Debug, Hash, PartialEq, Eq, Clone, Display, From)]
pub enum StatefulLocalUri {
    /// Stores [`LocalUri`] in [`ToRoom`] state.
    Room(LocalUri<ToRoom>),

    /// Stores [`LocalUri`] in [`ToMember`] state.
    Member(LocalUri<ToMember>),

    /// Stores [`LocalUri`] in [`ToEndpoint`] state.
    Endpoint(LocalUri<ToEndpoint>),
}

impl StatefulLocalUri {
    /// Returns reference to [`RoomId`].
    ///
    /// This is possible in any [`LocalUri`] state.
    #[inline]
    #[must_use]
    pub fn room_id(&self) -> &RoomId {
        match self {
            StatefulLocalUri::Room(uri) => uri.room_id(),
            StatefulLocalUri::Member(uri) => uri.room_id(),
            StatefulLocalUri::Endpoint(uri) => uri.room_id(),
        }
    }
}

impl TryFrom<String> for StatefulLocalUri {
    type Error = LocalUriParseError;

    #[allow(clippy::option_if_let_else)]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Err(LocalUriParseError::Empty);
        }

        let url = match Url::parse(&value) {
            Ok(url) => url,
            Err(err) => {
                return Err(LocalUriParseError::UrlParseErr(value, err))
            }
        };

        if url.scheme() != "local" {
            return Err(LocalUriParseError::NotLocal(value));
        }

        let room_uri = match url.host() {
            Some(host) => {
                let host = host.to_string();
                if host.is_empty() {
                    return Err(LocalUriParseError::MissingPaths(value));
                }
                LocalUri::<ToRoom>::new(host.into())
            }
            None => return Err(LocalUriParseError::MissingPaths(value)),
        };

        let mut path = match url.path_segments() {
            Some(path) => path,
            None => return Ok(room_uri.into()),
        };

        let member_id = path
            .next()
            .filter(|id| !id.is_empty())
            .map(|id| MemberId(id.to_string()));

        let endpoint_id = path
            .next()
            .filter(|id| !id.is_empty())
            .map(ToString::to_string);

        if path.next().is_some() {
            return Err(LocalUriParseError::TooManyPaths(value));
        }

        if let Some(member_id) = member_id {
            let member_uri = room_uri.push_member_id(member_id);
            if let Some(endpoint_id) = endpoint_id {
                Ok(member_uri.push_endpoint_id(endpoint_id.into()).into())
            } else {
                Ok(member_uri.into())
            }
        } else if endpoint_id.is_some() {
            Err(LocalUriParseError::MissingPaths(value))
        } else {
            Ok(room_uri.into())
        }
    }
}

#[cfg(test)]
mod specs {
    use super::*;

    #[test]
    fn parse_local_uri_to_room_element() {
        let local_uri =
            StatefulLocalUri::try_from(String::from("local://room_id"))
                .unwrap();
        if let StatefulLocalUri::Room(room) = local_uri {
            assert_eq!(room.take_room_id(), RoomId::from("room_id"));
        } else {
            unreachable!(
                "Local uri '{}' parsed to {:?} state but should be in \
                 IsRoomId state.",
                local_uri, local_uri
            );
        }
    }

    #[test]
    fn parse_local_uri_to_element_of_room() {
        let local_uri = StatefulLocalUri::try_from(String::from(
            "local://room_id/room_element_id",
        ))
        .unwrap();
        if let StatefulLocalUri::Member(member) = local_uri {
            let (element_id, room_uri) = member.take_member_id();
            assert_eq!(element_id, MemberId("room_element_id".to_string()));
            let room_id = room_uri.take_room_id();
            assert_eq!(room_id, RoomId::from("room_id"));
        } else {
            unreachable!(
                "Local URI '{}' parsed to {:?} state but should be in \
                 IsMemberId state.",
                local_uri, local_uri
            );
        }
    }

    #[test]
    fn parse_local_uri_to_endpoint() {
        let local_uri = StatefulLocalUri::try_from(String::from(
            "local://room_id/room_element_id/endpoint_id",
        ))
        .unwrap();
        if let StatefulLocalUri::Endpoint(endpoint) = local_uri {
            let (endpoint_id, member_uri) = endpoint.take_endpoint_id();
            assert_eq!(endpoint_id, String::from("endpoint_id").into());
            let (member_id, room_uri) = member_uri.take_member_id();
            assert_eq!(member_id, MemberId::from("room_element_id"));
            let room_id = room_uri.take_room_id();
            assert_eq!(room_id, RoomId::from("room_id"));
        } else {
            unreachable!(
                "Local URI '{}' parsed to {:?} state but should be in \
                 IsEndpointId state.",
                local_uri, local_uri
            );
        }
    }

    #[test]
    fn returns_parse_error_if_local_uri_not_local() {
        match StatefulLocalUri::try_from(String::from("not-local://room_id")) {
            Ok(_) => unreachable!(),
            Err(e) => match e {
                LocalUriParseError::NotLocal(_) => (),
                _ => unreachable!("Unreachable LocalUriParseError: {:?}", e),
            },
        }
    }

    #[test]
    fn returns_parse_error_if_local_uri_empty() {
        match StatefulLocalUri::try_from(String::from("")) {
            Ok(_) => unreachable!(),
            Err(e) => match e {
                LocalUriParseError::Empty => (),
                _ => unreachable!(),
            },
        }
    }

    #[test]
    fn returns_error_if_local_uri_have_too_many_paths() {
        match StatefulLocalUri::try_from(String::from(
            "local://room/member/endpoint/too_many",
        )) {
            Ok(_) => unreachable!(),
            Err(e) => match e {
                LocalUriParseError::TooManyPaths(_) => (),
                _ => unreachable!(),
            },
        }
    }

    #[test]
    fn properly_serialize() {
        for local_uri_str in [
            "local://room_id",
            "local://room_id/member_id",
            "local://room_id/member_id/endpoint_id",
        ] {
            let local_uri =
                StatefulLocalUri::try_from(local_uri_str.to_string()).unwrap();
            assert_eq!(local_uri_str, &local_uri.to_string());
        }
    }

    #[test]
    fn return_error_when_local_uri_not_full() {
        for local_uri_str in [
            "local://room_id//endpoint_id",
            "local:////endpoint_id",
            "local:///member_id/endpoint_id",
        ] {
            match StatefulLocalUri::try_from(local_uri_str.to_string()) {
                Ok(_) => unreachable!(),
                Err(e) => match e {
                    LocalUriParseError::MissingPaths(_) => (),
                    _ => unreachable!(),
                },
            }
        }
    }
}

//! URI for pointing to some medea element.

use std::{convert::TryFrom, fmt};

use derive_more::{Display, From};
use failure::Fail;
use url::Url;

use crate::api::control::endpoints::webrtc_play_endpoint::SrcUri;

use super::{MemberId, RoomId};

/// Error which can happen while [`LocalUri`] parsing.
#[allow(clippy::module_name_repetitions)]
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

/// Enum for store [`LocalUri`]s in all states.
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Hash, PartialEq, Eq, Clone, Display, From)]
pub enum StatefulLocalUri {
    /// Stores [`LocalUri`] in [`IsRoomId`] state.
    Room(LocalUri<IsRoomId>),

    /// Stores [`LocalUri`] in [`IsMemberId`] state.
    Member(LocalUri<IsMemberId>),

    /// Stores [`LocalUri`] in [`IsEndpointId`] state.
    Endpoint(LocalUri<IsEndpointId>),
}

impl StatefulLocalUri {
    /// Returns reference to [`RoomId`].
    ///
    /// This is possible in any [`LocalUri`] state.
    pub fn room_id(&self) -> &RoomId {
        match self {
            StatefulLocalUri::Room(uri) => uri.room_id(),
            StatefulLocalUri::Member(uri) => uri.room_id(),
            StatefulLocalUri::Endpoint(uri) => uri.room_id(),
        }
    }
}

impl TryFrom<&str> for StatefulLocalUri {
    type Error = LocalUriParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Err(LocalUriParseError::Empty);
        }

        let url = match Url::parse(value) {
            Ok(url) => url,
            Err(e) => {
                return Err(LocalUriParseError::UrlParseErr(
                    value.to_string(),
                    e,
                ));
            }
        };

        if url.scheme() != "local" {
            return Err(LocalUriParseError::NotLocal(value.to_string()));
        }

        let room_uri = url
            .host()
            .map(|id| id.to_string())
            .filter(|id| !id.is_empty())
            .map(|id| LocalUri::<IsRoomId>::new(id.into()))
            .ok_or_else(|| {
                LocalUriParseError::MissingPaths(value.to_string())
            })?;

        let mut path = match url.path_segments() {
            Some(path) => path,
            None => return Ok(Self::from(room_uri)),
        };

        let member_id = path
            .next()
            .filter(|id| !id.is_empty())
            .map(|id| MemberId(id.to_string()));

        let endpoint_id = path
            .next()
            .filter(|id| !id.is_empty())
            .map(|id| id.to_string());

        if path.next().is_some() {
            return Err(LocalUriParseError::TooManyPaths(value.to_string()));
        }

        if let Some(member_id) = member_id {
            let member_uri = room_uri.push_member_id(member_id);
            if let Some(endpoint_id) = endpoint_id {
                Ok(Self::from(member_uri.push_endpoint_id(endpoint_id)))
            } else {
                Ok(Self::from(member_uri))
            }
        } else if endpoint_id.is_some() {
            Err(LocalUriParseError::MissingPaths(value.to_string()))
        } else {
            Ok(Self::from(room_uri))
        }
    }
}

/// State of [`LocalUri`] which points to [`Room`].
///
/// [`Room`]: crate::signalling::room::Room
#[derive(Debug, PartialEq, Hash, Eq, Clone)]
pub struct IsRoomId(RoomId);

/// State of [`LocalUri`] which points to [`Member`].
///
/// [`Member`]: crate::signalling::elements::member::Member
#[derive(Debug, PartialEq, Hash, Eq, Clone)]
pub struct IsMemberId(LocalUri<IsRoomId>, MemberId);

/// State of [`LocalUri`] which points to [`Endpoint`].
///
/// [`Endpoint`]: crate::signalling::elements::endpoints::Endpoint
#[derive(Debug, PartialEq, Hash, Eq, Clone)]
pub struct IsEndpointId(LocalUri<IsMemberId>, String);

/// Uri in format `local://room_id/member_id/endpoint_id`.
///
/// This kind of uri used for pointing to some element in spec ([`Room`],
/// [`Member`], [`WebRtcPlayEndpoint`], [`WebRtcPublishEndpoint`], etc) based on
/// state.
///
/// [`LocalUri`] can be in three states: [`IsRoomId`], [`IsMemberId`],
/// [`IsRoomId`]. This is used for compile time guarantees that some
/// [`LocalUri`] have all mandatory fields.
///
/// You also can take value from [`LocalUri`] without copy, but you have to do
/// it consistently. For example, if you wish to get [`RoomId`], [`MemberId`]
/// and [`Endpoint`] ID from [`LocalUri`] in [`IsEndpointId`] state you should
/// make this steps:
///
/// ```
/// # use crate::api::control::local_uri::{LocalUri, IsEndpointId};
/// # use crate::api::control::{RoomId, MemberId};
/// #
/// let orig_room_id = RoomId("room".to_string());
/// let orig_member_id = MemberId("member".to_string());
/// let orig_endpoint_id = "endpoint".to_string();
///
/// // Create new LocalUri for endpoint.
/// let local_uri = LocalUri::<IsEndpointId>::new(
///     orig_room_id.clone(),
///     orig_member_id.clone(),
///     orig_endpoint_id.clone()
/// );
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
/// ```
///
/// This is necessary so that it is not possible to get the address in the
/// wrong state (`local://room_id//endpoint_id` for example).
///
/// [`Member`]: crate::signalling::elements::member::Member
/// [`Room`]: crate::signalling::room::Room
/// [`WebRtcPlayEndpoint`]:
/// crate::signalling::elements::endpoints::webrtc::WebRtcPlayEndpoint
/// [`WebRtcPublishEndpoint`]:
/// crate::signalling::elements::endpoints::webrtc::WebRtcPublishEndpoint
/// [`Endpoint`]: crate::signalling::elements::endpoints::Endpoint
#[derive(Debug, PartialEq, Hash, Eq, Clone)]
pub struct LocalUri<T> {
    state: T,
}

impl LocalUri<IsRoomId> {
    /// Create new [`LocalUri`] in [`IsRoomId`] state.
    pub fn new(room_id: RoomId) -> Self {
        Self {
            state: IsRoomId(room_id),
        }
    }

    /// Returns reference to [`RoomId`].
    pub fn room_id(&self) -> &RoomId {
        &self.state.0
    }

    /// Returns [`RoomId`].
    pub fn take_room_id(self) -> RoomId {
        self.state.0
    }

    /// Push [`MemberId`] to the end of URI and returns
    /// [`LocalUri`] in [`IsMemberId`] state.
    pub fn push_member_id(self, member_id: MemberId) -> LocalUri<IsMemberId> {
        LocalUri::<IsMemberId>::new(self.state.0, member_id)
    }
}

impl LocalUri<IsMemberId> {
    /// Create new [`LocalUri`] in [`IsMemberId`] state.
    pub fn new(room_id: RoomId, member_id: MemberId) -> Self {
        Self {
            state: IsMemberId(LocalUri::<IsRoomId>::new(room_id), member_id),
        }
    }

    /// Returns reference to [`RoomId`].
    pub fn room_id(&self) -> &RoomId {
        &self.state.0.room_id()
    }

    /// Returns reference to [`MemberId`].
    pub fn member_id(&self) -> &MemberId {
        &self.state.1
    }

    /// Return [`MemberId`] and [`LocalUri`] in state [`IsRoomId`].
    pub fn take_member_id(self) -> (MemberId, LocalUri<IsRoomId>) {
        (self.state.1, self.state.0)
    }

    /// Push endpoint ID to the end of URI and returns
    /// [`LocalUri`] in [`IsEndpointId`] state.
    pub fn push_endpoint_id(
        self,
        endpoint_id: String,
    ) -> LocalUri<IsEndpointId> {
        let (member_id, room_uri) = self.take_member_id();
        let room_id = room_uri.take_room_id();
        LocalUri::<IsEndpointId>::new(room_id, member_id, endpoint_id)
    }
}

impl LocalUri<IsEndpointId> {
    /// Creates new [`LocalUri`] in [`IsEndpointId`] state.
    pub fn new(
        room_id: RoomId,
        member_id: MemberId,
        endpoint_id: String,
    ) -> Self {
        Self {
            state: IsEndpointId(
                LocalUri::<IsMemberId>::new(room_id, member_id),
                endpoint_id,
            ),
        }
    }

    /// Returns reference to [`RoomId`].
    pub fn room_id(&self) -> &RoomId {
        &self.state.0.room_id()
    }

    /// Returns reference to [`MemberId`].
    pub fn member_id(&self) -> &MemberId {
        &self.state.0.member_id()
    }

    /// Returns reference to [`Endpoint`] ID.
    ///
    /// [`Endpoint`]: crate::signalling::elements::endpoints::Endpoint
    pub fn endpoint_id(&self) -> &str {
        &self.state.1
    }

    /// Returns [`Endpoint`] id and [`LocalUri`] in [`IsMemberId`] state.
    ///
    /// [`Endpoint`]: crate::signalling::elements::endpoints::Endpoint
    pub fn take_endpoint_id(self) -> (String, LocalUri<IsMemberId>) {
        (self.state.1, self.state.0)
    }
}

impl From<SrcUri> for LocalUri<IsEndpointId> {
    fn from(uri: SrcUri) -> Self {
        LocalUri::<IsEndpointId>::new(
            uri.room_id,
            uri.member_id,
            uri.endpoint_id.0,
        )
    }
}

impl fmt::Display for LocalUri<IsRoomId> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "local://{}", self.state.0)
    }
}

impl fmt::Display for LocalUri<IsMemberId> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.state.0, self.state.1)
    }
}

impl fmt::Display for LocalUri<IsEndpointId> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.state.0, self.state.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_local_uri_to_room_element() {
        let local_uri = StatefulLocalUri::try_from("local://room_id").unwrap();
        if let StatefulLocalUri::Room(room) = local_uri {
            assert_eq!(room.take_room_id(), RoomId("room_id".to_string()));
        } else {
            unreachable!("{:?}", local_uri);
        }
    }

    #[test]
    fn parse_local_uri_to_element_of_room() {
        let local_uri =
            StatefulLocalUri::try_from("local://room_id/room_element_id")
                .unwrap();
        if let StatefulLocalUri::Member(member) = local_uri.clone() {
            let (element_id, room_uri) = member.take_member_id();
            assert_eq!(element_id, MemberId("room_element_id".to_string()));
            let room_id = room_uri.take_room_id();
            assert_eq!(room_id, RoomId("room_id".to_string()));
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
        let local_uri = StatefulLocalUri::try_from(
            "local://room_id/room_element_id/endpoint_id",
        )
        .unwrap();
        if let StatefulLocalUri::Endpoint(endpoint) = local_uri.clone() {
            let (endpoint_id, member_uri) = endpoint.take_endpoint_id();
            assert_eq!(endpoint_id, "endpoint_id".to_string());
            let (member_id, room_uri) = member_uri.take_member_id();
            assert_eq!(member_id, MemberId("room_element_id".to_string()));
            let room_id = room_uri.take_room_id();
            assert_eq!(room_id, RoomId("room_id".to_string()));
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
        match StatefulLocalUri::try_from("not-local://room_id") {
            Ok(_) => unreachable!(),
            Err(e) => match e {
                LocalUriParseError::NotLocal(_) => (),
                _ => unreachable!("Unreachable LocalUriParseError: {:?}", e),
            },
        }
    }

    #[test]
    fn returns_parse_error_if_local_uri_empty() {
        match StatefulLocalUri::try_from("") {
            Ok(_) => unreachable!(),
            Err(e) => match e {
                LocalUriParseError::Empty => (),
                _ => unreachable!("Unreachable LocalUriParseError: {:?}", e),
            },
        }
    }

    #[test]
    fn returns_error_if_local_uri_have_too_many_paths() {
        match StatefulLocalUri::try_from(
            "local://room/member/endpoint/too_many",
        ) {
            Ok(_) => unreachable!(),
            Err(e) => match e {
                LocalUriParseError::TooManyPaths(_) => (),
                _ => unreachable!("Unreachable LocalUriParseError: {:?}", e),
            },
        }
    }

    #[test]
    fn properly_serialize() {
        for local_uri_str in vec![
            "local://room_id",
            "local://room_id/member_id",
            "local://room_id/member_id/endpoint_id",
        ] {
            let local_uri = StatefulLocalUri::try_from(local_uri_str).unwrap();
            assert_eq!(local_uri_str.to_string(), local_uri.to_string());
        }
    }

    #[test]
    fn return_error_when_local_uri_not_full() {
        for local_uri_str in vec![
            "local://room_id//endpoint_id",
            "local:////endpoint_id",
            "local:///member_id/endpoint_id",
        ] {
            match StatefulLocalUri::try_from(local_uri_str) {
                Ok(_) => unreachable!(local_uri_str),
                Err(e) => match e {
                    LocalUriParseError::MissingPaths(_) => (),
                    _ => unreachable!(
                        "Unreachable LocalUriParseError {:?} for uri '{}'",
                        e, local_uri_str
                    ),
                },
            }
        }
    }
}

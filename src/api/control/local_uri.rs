//! URI for pointing to some medea element.

// Bug in clippy.
#![allow(clippy::use_self)]

use std::fmt;

use failure::Fail;

use crate::api::{
    control::{endpoints::webrtc_play_endpoint::SrcUri, WebRtcPublishId},
    error_codes::ErrorCode,
};

use super::{MemberId, RoomId};

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Fail)]
pub enum LocalUriParseError {
    /// Protocol of provided URI is not "local://".
    #[fail(display = "Provided URIs protocol is not 'local://'.")]
    NotLocal(String),

    /// Too many paths in provided URI.
    #[fail(display = "Too many ({}) paths in provided URI.", _0)]
    TooManyFields(usize, String),

    #[fail(display = "Missing fields. {}", _0)]
    MissingFields(String),

    /// Provided empty `&str`.
    #[fail(display = "You provided empty local uri.")]
    Empty,
}

impl Into<ErrorCode> for LocalUriParseError {
    fn into(self) -> ErrorCode {
        match self {
            LocalUriParseError::NotLocal(text) => {
                ErrorCode::ElementIdIsNotLocal(text)
            }
            LocalUriParseError::TooManyFields(_, text) => {
                ErrorCode::ElementIdIsTooLong(text)
            }
            LocalUriParseError::Empty => ErrorCode::EmptyElementId,
            LocalUriParseError::MissingFields(text) => {
                ErrorCode::MissingFieldsInSrcUri(text)
            }
        }
    }
}

#[derive(Debug)]
pub struct IsRoomId(RoomId);
#[derive(Debug)]
pub struct IsMemberId(LocalUri<IsRoomId>, MemberId);
#[derive(Debug)]
pub struct IsEndpointId(LocalUri<IsMemberId>, String);

#[derive(Debug)]
pub struct LocalUri<T> {
    state: T,
}

impl LocalUriType {
    pub fn parse(value: &str) -> Result<Self, LocalUriParseError> {
        let inner = LocalUriInner::parse(value)?;
        if inner.is_room_uri() {
            Ok(LocalUriType::Room(LocalUri::<IsRoomId>::new(
                inner.room_id.unwrap(),
            )))
        } else if inner.is_member_uri() {
            Ok(LocalUriType::Member(LocalUri::<IsMemberId>::new(
                inner.room_id.unwrap(),
                inner.member_id.unwrap(),
            )))
        } else if inner.is_endpoint_uri() {
            Ok(LocalUriType::Endpoint(LocalUri::<IsEndpointId>::new(
                inner.room_id.unwrap(),
                inner.member_id.unwrap(),
                inner.endpoint_id.unwrap(),
            )))
        } else {
            Err(LocalUriParseError::MissingFields(value.to_string()))
        }
    }
}

impl LocalUri<IsRoomId> {
    pub fn new(room_id: RoomId) -> Self {
        Self {
            state: IsRoomId(room_id),
        }
    }

    pub fn room_id(&self) -> &RoomId {
        &self.state.0
    }

    pub fn take_room_id(self) -> RoomId {
        self.state.0
    }
}

impl LocalUri<IsMemberId> {
    pub fn new(room_id: RoomId, member_id: MemberId) -> Self {
        Self {
            state: IsMemberId(LocalUri::<IsRoomId>::new(room_id), member_id),
        }
    }

    pub fn room_id(&self) -> &RoomId {
        &self.state.0.room_id()
    }

    pub fn member_id(&self) -> &MemberId {
        &self.state.1
    }

    pub fn take_member_id(self) -> (MemberId, LocalUri<IsRoomId>) {
        (self.state.1, self.state.0)
    }
}

impl LocalUri<IsEndpointId> {
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

    pub fn room_id(&self) -> &RoomId {
        &self.state.0.room_id()
    }

    pub fn member_id(&self) -> &MemberId {
        &self.state.0.member_id()
    }

    pub fn endpoint_id(&self) -> &str {
        &self.state.1
    }

    pub fn take_endpoint_id(self) -> (String, LocalUri<IsMemberId>) {
        (self.state.1, self.state.0)
    }
}

impl Into<SrcUri> for LocalUri<IsEndpointId> {
    fn into(self) -> SrcUri {
        SrcUri {
            room_id: self.state.0.state.0.state.0,
            member_id: self.state.0.state.1,
            endpoint_id: WebRtcPublishId(self.state.1),
        }
    }
}

#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
pub enum LocalUriType {
    Room(LocalUri<IsRoomId>),
    Member(LocalUri<IsMemberId>),
    Endpoint(LocalUri<IsEndpointId>),
}

#[allow(clippy::doc_markdown)]
/// Uri in format "local://room_id/member_id/endpoint_id"
/// This kind of uri used for pointing to some element in spec (`Room`,
/// `Member`, `WebRtcPlayEndpoint`, `WebRtcPublishEndpoint`, etc).
#[derive(Debug, Clone)]
struct LocalUriInner {
    /// ID of [`Room`]
    room_id: Option<RoomId>,
    /// ID of `Member`
    member_id: Option<MemberId>,
    /// Control ID of [`Endpoint`]
    endpoint_id: Option<String>,
}

impl LocalUriInner {
    /// Parse [`LocalUri`] from str.
    ///
    /// Returns [`LocalUriParse::NotLocal`] when uri is not "local://"
    /// Returns [`LocalUriParse::TooManyFields`] when uri have too many paths.
    fn parse(value: &str) -> Result<Self, LocalUriParseError> {
        if value.is_empty() {
            return Err(LocalUriParseError::Empty);
        }
        let protocol_name: String = value.chars().take(8).collect();
        if protocol_name != "local://" {
            return Err(LocalUriParseError::NotLocal(value.to_string()));
        }

        let uri_body = value.chars().skip(8).collect::<String>();
        let mut uri_body_splitted: Vec<&str> = uri_body.rsplit('/').collect();
        let uri_body_splitted_len = uri_body_splitted.len();

        if uri_body_splitted_len > 3 {
            return Err(LocalUriParseError::TooManyFields(
                uri_body_splitted_len,
                value.to_string(),
            ));
        }

        let room_id = uri_body_splitted
            .pop()
            .filter(|p| !p.is_empty())
            .map(|p| RoomId(p.to_string()));
        let member_id = uri_body_splitted
            .pop()
            .filter(|p| !p.is_empty())
            .map(|p| MemberId(p.to_string()));
        let endpoint_id = uri_body_splitted
            .pop()
            .filter(|p| !p.is_empty())
            .map(std::string::ToString::to_string);

        Ok(Self {
            room_id,
            member_id,
            endpoint_id,
        })
    }

    /// Return true if this [`LocalUri`] pointing to `Room` element.
    fn is_room_uri(&self) -> bool {
        self.room_id.is_some()
            && self.member_id.is_none()
            && self.endpoint_id.is_none()
    }

    /// Return true if this [`LocalUri`] pointing to `Member` element.
    fn is_member_uri(&self) -> bool {
        self.room_id.is_some()
            && self.member_id.is_some()
            && self.endpoint_id.is_none()
    }

    /// Return true if this [`LocalUri`] pointing to `Endpoint` element.
    fn is_endpoint_uri(&self) -> bool {
        self.room_id.is_some()
            && self.member_id.is_some()
            && self.endpoint_id.is_some()
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

impl fmt::Display for LocalUriType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LocalUriType::Room(e) => write!(f, "{}", e),
            LocalUriType::Member(e) => write!(f, "{}", e),
            LocalUriType::Endpoint(e) => write!(f, "{}", e),
        }
    }
}

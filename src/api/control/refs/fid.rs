/// Implementation of Full ID (`fid` in dynamic Control API specs).

use std::{
    convert::{From, TryFrom},
    fmt::{Display, Error, Formatter},
};

use derive_more::{Display, From};

use crate::{api::control::RoomId, impl_uri};

use super::{ToEndpoint, ToMember, ToRoom};

/// Errors which can happen while parsing [`Fid`].
#[derive(Display, Debug)]
pub enum ParseFidError {
    #[display(fmt = "Fid is empty.")]
    Empty,

    #[display(fmt = "Too many paths [id = {}].", _0)]
    TooManyPaths(String),
}

/// Full ID (`fid` in dynamic Control API specs).
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Fid<T> {
    state: T,
}

impl_uri!(Fid);

impl From<StatefulFid> for Fid<ToRoom> {
    fn from(from: StatefulFid) -> Self {
        match from {
            StatefulFid::Room(uri) => uri,
            StatefulFid::Member(uri) => {
                let (_, uri) = uri.take_member_id();
                uri
            }
            StatefulFid::Endpoint(uri) => {
                let (_, uri) = uri.take_endpoint_id();
                let (_, uri) = uri.take_member_id();
                uri
            }
        }
    }
}

impl Display for Fid<ToRoom> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{}", self.state.0)
    }
}

impl Display for Fid<ToMember> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{}/{}", self.state.0, self.state.1)
    }
}

impl Display for Fid<ToEndpoint> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{}/{}/{}", self.state.0, self.state.1, self.state.2)
    }
}

/// Enum for storing [`Fid`]s in all states.
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Hash, PartialEq, Eq, Clone, Display, From)]
pub enum StatefulFid {
    Room(Fid<ToRoom>),
    Member(Fid<ToMember>),
    Endpoint(Fid<ToEndpoint>),
}

impl StatefulFid {
    /// Returns reference to [`RoomId`].
    ///
    /// This is possible in any [`LocalUri`] state.
    pub fn room_id(&self) -> &RoomId {
        match self {
            StatefulFid::Room(uri) => uri.room_id(),
            StatefulFid::Member(uri) => uri.room_id(),
            StatefulFid::Endpoint(uri) => uri.room_id(),
        }
    }
}

impl TryFrom<String> for StatefulFid {
    type Error = ParseFidError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Err(ParseFidError::Empty);
        }

        let mut splitted = value.split('/');
        let room_id = if let Some(room_id) = splitted.next() {
            room_id
        } else {
            return Err(ParseFidError::Empty);
        };

        let member_id = if let Some(member_id) = splitted.next() {
            member_id
        } else {
            return Ok(Fid::<ToRoom>::new(room_id.to_string().into()).into());
        };

        let endpoint_id = if let Some(endpoint_id) = splitted.next() {
            endpoint_id
        } else {
            return Ok(Fid::<ToMember>::new(
                room_id.to_string().into(),
                member_id.to_string().into(),
            )
            .into());
        };

        if splitted.next().is_some() {
            Err(ParseFidError::TooManyPaths(value))
        } else {
            Ok(Fid::<ToEndpoint>::new(
                room_id.to_string().into(),
                member_id.to_string().into(),
                endpoint_id.to_string().into(),
            )
            .into())
        }
    }
}

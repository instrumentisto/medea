use std::{
    convert::TryFrom,
    fmt::{Display, Error, Formatter},
};

use derive_more::{Display, From};

use crate::impl_uri;

use super::{ToEndpoint, ToMember, ToRoom};
use crate::api::control::RoomId;
use std::convert::From;

#[derive(Display, Debug)]
pub enum ParseFidError {
    #[display(fmt = "Fid is empty.")]
    Empty,

    #[display(fmt = "Too many paths [id = {}].", _0)]
    TooManyPaths(String),
}

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

        if let Some(_) = splitted.next() {
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

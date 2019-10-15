use std::{
    convert::TryFrom,
    fmt::{Display, Error, Formatter},
};

use derive_more::{Display, From};

use crate::impl_uri;

use super::{ToEndpoint, ToMember, ToRoom};

#[derive(Display)]
pub enum ParseFidError {
    #[display(fmt = "Fid is empty.")]
    Empty,

    #[display(fmt = "Too many paths.")]
    TooManyPaths,
}

pub struct Fid<T> {
    state: T,
}

impl_uri!(Fid);

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

#[derive(From)]
pub enum StatefulFid {
    Room(Fid<ToRoom>),
    Member(Fid<ToMember>),
    Endpoint(Fid<ToEndpoint>),
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
            Err(ParseFidError::TooManyPaths)
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

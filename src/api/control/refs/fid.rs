//! Implementation of Full ID (`fid` in dynamic Control API specs).

use std::{
    convert::{From, TryFrom},
    fmt::{Display, Error, Formatter},
};

use derive_more::{Display, From};
use failure::Fail;
use medea_macro::enum_delegate;

use crate::{
    api::control::{EndpointId, MemberId, RoomId},
    impls_for_stateful_refs,
};

use super::{ToEndpoint, ToMember, ToRoom};

/// Errors which can happen while parsing [`Fid`].
#[derive(Display, Debug, Fail)]
pub enum ParseFidError {
    #[display(fmt = "Fid is empty.")]
    Empty,

    #[display(fmt = "Too many paths [fid = {}].", _0)]
    TooManyPaths(String),

    #[display(fmt = "Missing paths [fid = {}]", _0)]
    MissingPath(String),
}

/// FID (full ID, or `fid` in Control API specs) is a composition of
/// media elements IDs, which refers to some media element on a whole server
/// uniquely.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Fid<T> {
    state: T,
}

impls_for_stateful_refs!(Fid);

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
#[enum_delegate(pub fn room_id(&self) -> &RoomId)]
#[derive(Debug, Hash, PartialEq, Eq, Clone, Display, From)]
pub enum StatefulFid {
    Room(Fid<ToRoom>),
    Member(Fid<ToMember>),
    Endpoint(Fid<ToEndpoint>),
}

impl TryFrom<String> for StatefulFid {
    type Error = ParseFidError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Err(ParseFidError::Empty);
        }

        let mut splitted = value.split('/');
        let room_id = if let Some(room_id) = splitted.next() {
            if room_id.is_empty() {
                return Err(ParseFidError::MissingPath(value));
            } else {
                room_id
            }
        } else {
            return Err(ParseFidError::Empty);
        };

        let member_id = if let Some(member_id) = splitted.next() {
            if member_id.is_empty() {
                return Err(ParseFidError::MissingPath(value));
            } else {
                member_id
            }
        } else {
            return Ok(Fid::<ToRoom>::new(RoomId::from(room_id)).into());
        };

        let endpoint_id = if let Some(endpoint_id) = splitted.next() {
            if endpoint_id.is_empty() {
                return Err(ParseFidError::MissingPath(value));
            } else {
                endpoint_id
            }
        } else {
            return Ok(Fid::<ToMember>::new(
                RoomId::from(room_id),
                MemberId::from(member_id),
            )
            .into());
        };

        if splitted.next().is_some() {
            Err(ParseFidError::TooManyPaths(value))
        } else {
            Ok(Fid::<ToEndpoint>::new(
                RoomId::from(room_id),
                MemberId::from(member_id),
                EndpointId::from(endpoint_id),
            )
            .into())
        }
    }
}

#[cfg(test)]
mod specs {
    use crate::api::control::{EndpointId, MemberId};

    use super::*;

    #[test]
    fn returns_error_on_missing_path() {
        for fid_str in &[
            "room_id//endpoint_id",
            "//endpoint_id",
            "//member_id/endpoint_id",
            "/member_id",
        ] {
            match StatefulFid::try_from((*fid_str).to_string()) {
                Ok(f) => unreachable!("Unexpected successful parse: {}", f),
                Err(e) => match e {
                    ParseFidError::MissingPath(_) => (),
                    _ => unreachable!("Throwed some unexpected error {:?}.", e),
                },
            }
        }
    }

    #[test]
    fn returns_error_on_too_many_paths() {
        for fid_str in &[
            "room_id/member_id/endpoint_id/something_else",
            "room_id/member_id/endpoint_id/",
            "room_id/member_id/endpoint_id////",
        ] {
            match StatefulFid::try_from((*fid_str).to_string()) {
                Ok(f) => unreachable!("Unexpected successful parse: {}", f),
                Err(e) => match e {
                    ParseFidError::TooManyPaths(_) => (),
                    _ => unreachable!("Throwed some unexpected error {:?}.", e),
                },
            }
        }
    }

    #[test]
    fn successful_parse_to_room() {
        let room_id = RoomId::from("room_id");
        let fid = StatefulFid::try_from(format!("{}", room_id)).unwrap();
        match fid {
            StatefulFid::Room(room_fid) => {
                assert_eq!(room_fid.room_id(), &room_id);
            }
            _ => unreachable!("Fid parsed not to Room. {}", fid),
        }
    }

    #[test]
    fn successful_parse_to_member() {
        let room_id = RoomId::from("room_id");
        let member_id = MemberId::from("member_id");
        let fid = StatefulFid::try_from(format!("{}/{}", room_id, member_id))
            .unwrap();

        match fid {
            StatefulFid::Member(member_fid) => {
                assert_eq!(member_fid.room_id(), &room_id);
                assert_eq!(member_fid.member_id(), &member_id);
            }
            _ => unreachable!("Fid parsed not to Member. {}", fid),
        }
    }

    #[test]
    fn successful_parse_to_endpoint() {
        let room_id = RoomId::from("room_id");
        let member_id = MemberId::from("member_id");
        let endpoint_id = EndpointId::from("endpoint_id");
        let fid = StatefulFid::try_from(format!(
            "{}/{}/{}",
            room_id, member_id, endpoint_id
        ))
        .unwrap();

        match fid {
            StatefulFid::Endpoint(endpoint_fid) => {
                assert_eq!(endpoint_fid.room_id(), &room_id);
                assert_eq!(endpoint_fid.member_id(), &member_id);
                assert_eq!(endpoint_fid.endpoint_id(), &endpoint_id);
            }
            _ => unreachable!("Fid parsed not to Member. {}", fid),
        }
    }

    #[test]
    fn serializes_into_original_fid() {
        for fid_str in &[
            "room_id",
            "room_id/member_id",
            "room_id/member_id/endpoint_id",
        ] {
            let fid = StatefulFid::try_from((*fid_str).to_string()).unwrap();
            assert_eq!((*fid_str).to_string(), fid.to_string());
        }
    }
}

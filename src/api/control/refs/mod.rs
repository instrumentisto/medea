//! Implementation of all kinds of references to some resource used in Medea's
//! Control API.

#![allow(clippy::use_self)]

#[macro_use]
mod impls_for_stateful_refs;
pub mod fid;
pub mod local_uri;
pub mod src_uri;

use medea_client_api_proto::{MemberId, RoomId};

use super::EndpointId;

#[doc(inline)]
pub use self::{
    fid::{Fid, StatefulFid},
    local_uri::{LocalUri, StatefulLocalUri},
    src_uri::SrcUri,
};

/// State of reference which points to [`Room`].
///
/// [`Room`]: crate::signalling::room::Room
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ToRoom(RoomId);

/// State of reference which points to [`Member`].
///
/// [`Member`]: crate::signalling::elements::Member
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ToMember(RoomId, MemberId);

/// State of reference which points to [`Endpoint`].
///
/// [`Endpoint`]: crate::signalling::elements::endpoints::Endpoint
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ToEndpoint(RoomId, MemberId, EndpointId);

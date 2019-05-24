//! Implementation of Control API.

pub mod element;
pub mod member;
pub mod pipeline;
pub mod room;

use failure::{Error, Fail};
use serde::Deserialize;

use std::{convert::TryFrom as _, fs::File, io::Read as _};

use self::{
    element::{WebRtcPlayEndpoint, WebRtcPublishEndpoint},
    pipeline::Pipeline,
    room::RoomSpec,
};

pub use self::member::{Id as MemberId, Member};

/// Errors that can occur when we try transform some spec from [`Entity`].
/// This error used in all [`TryFrom`] of Control API.
#[derive(Debug, Fail)]
pub enum TryFromEntityError {
    #[fail(display = "This entity is not Element")]
    NotElement,
    #[fail(display = "This entity is not Room")]
    NotRoom,
    #[fail(display = "This entity is not Member")]
    NotMember,
}

/// Entity for parsing Control API request.
#[derive(Clone, Deserialize, Debug)]
#[serde(tag = "kind")]
pub enum Entity {
    /// Represent [`RoomSpec`].
    /// Can transform into [`RoomSpec`] by `RoomSpec::try_from`.
    Room { id: u64, spec: Pipeline },

    /// Represent [`MemberSpec`].
    /// Can transform into [`MemberSpec`] by `MemberSpec::try_from`.
    Member { spec: Pipeline },

    /// Represent [`WebRtcPublishEndpoint`].
    /// Can transform into [`Element`] enum by `Element::try_from`.
    WebRtcPublishEndpoint { spec: WebRtcPublishEndpoint },

    /// Represent [`WebRtcPlayEndpoint`].
    /// Can transform into [`Element`] enum by `Element::try_from`.
    WebRtcPlayEndpoint { spec: WebRtcPlayEndpoint },
}

/// Load [`Entity`] from file with YAML format.
pub fn load_from_yaml_file(path: &str) -> Result<RoomSpec, Error> {
    let mut file = File::open(path)?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)?;
    let parsed: Entity = serde_yaml::from_str(&buf)?;
    let room = RoomSpec::try_from(parsed)?;

    Ok(room)
}

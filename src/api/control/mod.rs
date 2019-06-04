//! Implementation of Control API.

pub mod element;
pub mod member;
pub mod pipeline;
pub mod room;

use failure::{Error, Fail};
use serde::Deserialize;

use std::{convert::TryFrom as _, fs::File, io::Read as _, path::Path};

use self::{
    element::{WebRtcPlayEndpoint, WebRtcPublishEndpoint},
    pipeline::Pipeline,
};

pub use self::{
    element::Endpoint,
    member::{Id as MemberId, Member, MemberSpec},
    room::{Id as RoomId, RoomSpec},
};

/// Errors that can occur when we try transform some spec from [`Entity`].
/// This error used in all [`TryFrom`] of Control API.
#[allow(clippy::pub_enum_variant_names)]
#[derive(Debug, Fail)]
pub enum TryFromEntityError {
    #[fail(display = "Entity is not Element")]
    NotElement,
    #[fail(display = "Entity is not Room")]
    NotRoom,
    #[fail(display = "Entity is not Member")]
    NotMember,
}

/// Entity for parsing Control API request.
#[derive(Clone, Deserialize, Debug)]
#[serde(tag = "kind")]
pub enum Entity {
    /// Represent [`RoomSpec`].
    /// Can transform into [`RoomSpec`] by `RoomSpec::try_from`.
    Room { id: RoomId, spec: Pipeline },

    /// Represent [`MemberSpec`].
    /// Can transform into [`MemberSpec`] by `MemberSpec::try_from`.
    Member { spec: Pipeline, credentials: String },

    /// Represent [`WebRtcPublishEndpoint`].
    /// Can transform into [`Element`] enum by `Element::try_from`.
    WebRtcPublishEndpoint { spec: WebRtcPublishEndpoint },

    /// Represent [`WebRtcPlayEndpoint`].
    /// Can transform into [`Element`] enum by `Element::try_from`.
    WebRtcPlayEndpoint { spec: WebRtcPlayEndpoint },
}

/// Load [`RoomSpec`] from file with YAML format.
pub fn load_from_yaml_file<P: AsRef<Path>>(path: P) -> Result<RoomSpec, Error> {
    let mut file = File::open(path)?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)?;
    let parsed: Entity = serde_yaml::from_str(&buf)?;
    let room = RoomSpec::try_from(parsed)?;

    Ok(room)
}

/// Load all [`RoomSpec`] from YAML files from provided path.
pub fn load_static_specs_from_dir<P: AsRef<Path>>(
    path: P,
) -> Result<Vec<RoomSpec>, Error> {
    let mut specs = Vec::new();
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let spec = load_from_yaml_file(entry.path())?;
        specs.push(spec)
    }

    Ok(specs)
}

//! Implementation of Control API.

pub mod element;
pub mod member;
pub mod room;

mod pipeline;

use failure::Error;
use failure::Fail;
use serde::Deserialize;
use std::{convert::TryFrom as _, fs::File, io::Read as _};

use self::{
    element::{WebRtcPlayEndpoint, WebRtcPublishEndpoint},
    pipeline::Pipeline,
    room::RoomSpec,
};

pub use self::member::{Id as MemberId, Member};

#[derive(Debug, Fail)]
pub enum TryFromEntityError {
    #[fail(display = "This entity is not Element")]
    NotElement,
    #[fail(display = "This entity is not Room")]
    NotRoom,
    #[fail(display = "This entity is not Member")]
    NotMember,
}

#[derive(Clone, Deserialize, Debug)]
#[serde(tag = "kind")]
pub enum Entity {
    Room { id: u64, spec: Pipeline },
    Member { spec: Pipeline },
    WebRtcPublishEndpoint { spec: WebRtcPublishEndpoint },
    WebRtcPlayEndpoint { spec: WebRtcPlayEndpoint },
}

/// Load [`RoomRequest`] from file with YAML format.
pub fn load_from_file(path: &str) -> Result<RoomSpec, Error> {
    let mut file = File::open(path)?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)?;
    let parsed: Entity = serde_yaml::from_str(&buf)?;
    let room = RoomSpec::try_from(parsed)?;

    Ok(room)
}

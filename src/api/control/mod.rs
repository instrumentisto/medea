//! Implementation of Control API.

mod element;
mod member;

pub mod room;

use failure::Error;
use serde::{Deserialize, Serialize};
use std::{fs::File, io::Read as _};

use crate::signalling::RoomId;

use self::room::RoomSpec;

pub use self::member::{Id as MemberId, Member};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "kind")]
/// Entity for creating new Room.
pub enum RoomRequest {
    Room { id: RoomId, spec: RoomSpec },
}

/// Load [`RoomRequest`] from file with YAML format.
pub fn load_from_file(path: &str) -> Result<RoomRequest, Error> {
    let mut file = File::open(path)?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)?;
    let parsed: RoomRequest = serde_yaml::from_str(&buf)?;

    Ok(parsed)
}

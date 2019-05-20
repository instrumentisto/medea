//! Implementation of Control API.

mod element;
mod member;
mod room;

use actix::Addr;
use failure::Error;
use serde::{Deserialize, Serialize};
use std::{fs::File, io::Read as _};

use crate::signalling::{Room, RoomId};

use self::room::RoomSpec;

pub use self::member::{Id as MemberId, Member};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "kind")]
pub enum RoomRequest {
    Room { id: RoomId, spec: RoomSpec },
}

pub fn load_from_file(path: &str) -> Result<RoomRequest, Error> {
    let mut file = File::open(path)?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)?;
    let parsed: RoomRequest = serde_yaml::from_str(&buf)?;

    Ok(parsed)
}

#[derive(Clone)]
pub struct ControlRoom {
    pub client_room: Addr<Room>,
    pub spec: RoomSpec,
}

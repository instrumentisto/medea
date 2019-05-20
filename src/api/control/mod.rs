//! Implementation of Control API.

mod member;
mod room;
mod element;

use std::{
    fs::File,
    io::Read as _,
};
use failure::Error;
use serde::{Serialize, Deserialize};

use self::room::RoomSpec;

pub use self::member::{Id as MemberId, Member};

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "kind")]
pub enum ControlRoot {
    Room {
        id: String,
        spec: RoomSpec,
    },
}

pub fn load_from_file(path: &str) -> Result<ControlRoot, Error> {
    let mut file = File::open(path)?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)?;
    let parsed: ControlRoot = serde_yaml::from_str(&buf)?;

    Ok(parsed)
}

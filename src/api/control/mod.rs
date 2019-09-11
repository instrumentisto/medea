//! Implementation and definitions of [Control API] specs.
//!
//! [Control API]: http://tiny.cc/380uaz

pub mod endpoint;
pub mod member;
pub mod pipeline;
pub mod room;

use std::{
    convert::TryFrom as _,
    fs::{File, ReadDir},
    io::Read as _,
    path::Path,
};

use derive_more::Display;
use failure::{Error, Fail};
use serde::Deserialize;

use self::pipeline::Pipeline;

pub use self::{
    endpoint::Endpoint,
    member::{Id as MemberId, MemberSpec},
    room::{Id as RoomId, RoomElement, RoomSpec},
};

/// Root elements of [Control API] spec.
///
/// [Control API]: http://tiny.cc/380uaz
#[derive(Clone, Deserialize, Debug)]
#[serde(tag = "kind")]
pub enum RootElement {
    /// Represent [`RoomSpec`].
    /// Can transform into [`RoomSpec`] by `RoomSpec::try_from`.
    Room {
        id: RoomId,
        spec: Pipeline<RoomElement>,
    },
}

/// Errors that can occur when we try transform some spec from `Element`.
/// This error used in all [`TryFrom`] of Control API.
///
/// [`TryFrom`]: std::convert::TryFrom
#[allow(clippy::pub_enum_variant_names)]
#[derive(Debug, Display, Fail)]
pub enum TryFromElementError {
    #[display(fmt = "Element is not Room")]
    NotRoom,
    #[display(fmt = "Element is not Room")]
    NotMember,
}

/// Loads [`RoomSpec`] from file with YAML format.
pub fn load_from_yaml_file<P: AsRef<Path>>(path: P) -> Result<RoomSpec, Error> {
    let mut file = File::open(path)?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)?;
    let parsed: RootElement = serde_yaml::from_str(&buf)?;
    let room = RoomSpec::try_from(&parsed)?;
    Ok(room)
}

/// Loads all [`RoomSpec`] from YAML files from provided [`ReadDir`].
pub fn load_static_specs_from_dir(
    dir: ReadDir,
) -> Result<Vec<RoomSpec>, Error> {
    let mut specs = Vec::new();
    for entry in dir {
        let entry = entry?;
        let spec = load_from_yaml_file(entry.path())?;
        specs.push(spec)
    }
    Ok(specs)
}

//! Implementation and definitions of [Control API] specs.
//!
//! [Control API]: http://tiny.cc/380uaz

pub mod endpoints;
pub mod grpc;
pub mod local_uri;
pub mod member;
pub mod pipeline;
pub mod room;

use std::{convert::TryFrom as _, fs::File, io::Read as _, path::Path};

use derive_more::Display;
use failure::Fail;
use serde::Deserialize;

use self::{
    endpoints::webrtc_play_endpoint::SrcParseError, pipeline::Pipeline,
};

#[doc(inline)]
pub use self::{
    endpoints::{
        webrtc_play_endpoint::WebRtcPlayId,
        webrtc_publish_endpoint::WebRtcPublishId, Endpoint,
    },
    member::{Id as MemberId, MemberSpec},
    room::{Id as RoomId, RoomElement, RoomSpec},
};

/// Errors which may occur while deserialize protobuf spec.
#[derive(Debug, Fail, Display)]
pub enum TryFromProtobufError {
    /// Error while parsing src uri of [`WebRtcPlayEndpoint`].
    ///
    /// [`WebRtcPlayEndpoint`]:
    /// crate::api::control::endpoints::WebRtcPlayEndpoint
    #[display(fmt = "Src uri parse error: {:?}", _0)]
    SrcUriError(SrcParseError),

    /// Room element doesn't have Member element. Currently this is
    /// unimplemented.
    #[display(
        fmt = "Room element [id = {}]doesn't have Member element. Currently \
               this is unimplemented.",
        _0
    )]
    NotMemberElementInRoomElement(String),
}

impl From<SrcParseError> for TryFromProtobufError {
    fn from(from: SrcParseError) -> Self {
        TryFromProtobufError::SrcUriError(from)
    }
}

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
#[derive(Debug, Fail, Clone)]
pub enum TryFromElementError {
    /// Element is not Room.
    #[fail(display = "Element is not Room")]
    NotRoom,

    /// Element is not Member.
    #[fail(display = "Element is not Member")]
    NotMember,
}

/// Errors which can happen while loading static [Control API] specs.
#[derive(Debug, Fail, Display)]
pub enum LoadStaticControlSpecsError {
    /// Error while reading default or provided in config static [Control API]
    /// specs dir.
    ///
    /// Atm we only should print `warn!` message to log which prints that
    /// static specs not loaded.
    ///
    /// [Control API]: http://tiny.cc/380uaz
    #[display(fmt = "Error while reading static control API specs dir.")]
    SpecDirReadError(std::io::Error),

    /// I/O error while reading static [Control API] specs.
    ///
    /// [Control API]: http://tiny.cc/380uaz
    #[display(fmt = "I/O error while reading specs. {:?}", _0)]
    IoError(std::io::Error),

    /// Conflict in static [Control API] specs.
    ///
    /// [Control API]: http://tiny.cc/380uaz
    #[display(
        fmt = "Try from element error while loading static specs. {:?}",
        _0
    )]
    TryFromElementError(TryFromElementError),

    /// Error while deserialization static [Control API] specs from YAML file.
    ///
    /// [Control API]: http://tiny.cc/380uaz
    #[display(fmt = "Error while deserialization static spec. {:?}", _0)]
    YamlDeserializationError(serde_yaml::Error),
}

impl From<std::io::Error> for LoadStaticControlSpecsError {
    fn from(err: std::io::Error) -> Self {
        Self::IoError(err)
    }
}

impl From<TryFromElementError> for LoadStaticControlSpecsError {
    fn from(err: TryFromElementError) -> Self {
        Self::TryFromElementError(err)
    }
}

impl From<serde_yaml::Error> for LoadStaticControlSpecsError {
    fn from(err: serde_yaml::Error) -> Self {
        Self::YamlDeserializationError(err)
    }
}

/// Load [`RoomSpec`] from file with YAML format.
pub fn load_from_yaml_file<P: AsRef<Path>>(
    path: P,
) -> Result<RoomSpec, LoadStaticControlSpecsError> {
    let mut file = File::open(path)?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)?;
    let parsed: RootElement = serde_yaml::from_str(&buf)?;
    let room = RoomSpec::try_from(&parsed)?;
    Ok(room)
}

/// Load all [`RoomSpec`] from YAML files from provided path.
pub fn load_static_specs_from_dir<P: AsRef<Path>>(
    path: P,
) -> Result<Vec<RoomSpec>, LoadStaticControlSpecsError> {
    let mut specs = Vec::new();
    for entry in std::fs::read_dir(path)
        .map_err(LoadStaticControlSpecsError::SpecDirReadError)?
    {
        let entry = entry?;
        let spec = load_from_yaml_file(entry.path())?;
        specs.push(spec)
    }
    Ok(specs)
}

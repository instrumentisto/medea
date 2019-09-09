//! Implementation and definitions of [Control API] specs.
//!
//! [Control API]: http://tiny.cc/380uaz

pub mod endpoints;
pub mod grpc;
pub mod local_uri;
pub mod member;
pub mod pipeline;
pub mod room;

use std::{
    convert::TryFrom as _,
    fs::{File, ReadDir},
    io::Read as _,
    path::Path,
};

use derive_more::{Display, From};
use failure::{Error, Fail};
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
#[derive(Debug, Fail)]
pub enum TryFromProtobufError {
    /// Error while parsing src uri of [`WebRtcPlayEndpoint`].
    #[fail(display = "Src uri parse error: {:?}", _0)]
    SrcUriError(SrcParseError),

    /// Src URI not provided for [`WebRtcPlayEndpoint`].
    #[fail(display = "Src uri for publish endpoint not provided.")]
    SrcUriNotFound,

    /// Room element not provided.
    #[fail(display = "Room element not provided.")]
    RoomElementNotFound,

    /// Member element not provided.
    #[fail(display = "Member element not provided.")]
    MemberElementNotFound,

    /// [`P2pMode`] not found.
    #[fail(display = "P2p mode for play endpoint not provided.")]
    P2pModeNotFound,

    /// Member credentials not found.
    #[fail(display = "Credentials for member not provided.")]
    MemberCredentialsNotFound,
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

#[derive(From, Debug, Fail, Display)]
pub enum LoadStaticControlSpecsError {
    #[display(fmt = "Directory with specs not found.")]
    SpecDirNotFound,
    #[display(fmt = "I/O error while reading specs. {:?}", _0)]
    IoError(std::io::Error),
    #[display(
        fmt = "Try from element error while loading static specs. {:?}",
        _0
    )]
    TryFromElementError(TryFromElementError),
    #[display(fmt = "Error while deserialization static spec. {:?}", _0)]
    YamlDeserializationError(serde_yaml::Error),
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
        .map_err(|_| LoadStaticControlSpecsError::SpecDirNotFound)?
    {
        let entry = entry?;
        let spec = load_from_yaml_file(entry.path())?;
        specs.push(spec)
    }
    Ok(specs)
}

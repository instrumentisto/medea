//! Implementation of Control API.

// pub mod endpoint;
pub mod endpoints;
pub mod grpc;
pub mod local_uri;
pub mod member;
pub mod pipeline;
pub mod room;

use std::{convert::TryFrom as _, fs::File, io::Read as _, path::Path};

use failure::{Error, Fail};
use serde::Deserialize;

use self::{
    endpoints::{
        webrtc_play_endpoint::{SrcParseError, WebRtcPlayEndpoint},
        webrtc_publish_endpoint::WebRtcPublishEndpoint,
    },
    pipeline::Pipeline,
};

pub use self::{
    endpoints::{
        webrtc_play_endpoint::WebRtcPlayId,
        webrtc_publish_endpoint::WebRtcPublishId, Endpoint,
    },
    member::{Id as MemberId, MemberSpec},
    room::{Id as RoomId, RoomSpec},
};

#[derive(Debug, Fail)]
pub enum TryFromProtobufError {
    #[fail(display = "Src uri parse error: {:?}", _0)]
    SrcUriError(SrcParseError),
    #[fail(display = "Src uri for publish endpoint not provided.")]
    SrcUriNotFound,
    #[fail(display = "Room element not provided.")]
    RoomElementNotFound,
    #[fail(display = "Member element not provided.")]
    MemberElementNotFound,
    #[fail(display = "P2p mode for play endpoint not provided.")]
    P2pModeNotFound,
    #[fail(display = "Credentials for member not provided.")]
    MemberCredentialsNotFound,
}

impl From<SrcParseError> for TryFromProtobufError {
    fn from(from: SrcParseError) -> Self {
        TryFromProtobufError::SrcUriError(from)
    }
}

/// Errors that can occur when we try transform some spec from [`Element`].
/// This error used in all [`TryFrom`] of Control API.
#[allow(clippy::pub_enum_variant_names)]
#[derive(Debug, Fail)]
pub enum TryFromElementError {
    #[fail(display = "Element is not Endpoint")]
    NotEndpoint,
    #[fail(display = "Element is not Room")]
    NotRoom,
    #[fail(display = "Element is not Member")]
    NotMember,
}

/// Entity for parsing Control API request.
#[derive(Clone, Deserialize, Debug)]
#[serde(tag = "kind")]
pub enum Element {
    /// Represent [`RoomSpec`].
    /// Can transform into [`RoomSpec`] by `RoomSpec::try_from`.
    Room { id: RoomId, spec: Pipeline },

    /// Represent [`MemberSpec`].
    /// Can transform into [`MemberSpec`] by `MemberSpec::try_from`.
    Member { spec: Pipeline, credentials: String },

    /// Represent [`WebRtcPublishEndpoint`].
    /// Can transform into [`Endpoint`] enum by `Endpoint::try_from`.
    WebRtcPublishEndpoint { spec: WebRtcPublishEndpoint },

    /// Represent [`WebRtcPlayEndpoint`].
    /// Can transform into [`Endpoint`] enum by `Endpoint::try_from`.
    WebRtcPlayEndpoint { spec: WebRtcPlayEndpoint },
}

/// Load [`RoomSpec`] from file with YAML format.
pub fn load_from_yaml_file<P: AsRef<Path>>(path: P) -> Result<RoomSpec, Error> {
    let mut file = File::open(path)?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)?;
    let parsed: Element = serde_yaml::from_str(&buf)?;
    let room = RoomSpec::try_from(&parsed)?;

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

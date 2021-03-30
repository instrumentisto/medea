//! Implementation and definitions of [Control API] specs.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

pub mod callback;
pub mod endpoints;
pub mod error_codes;
pub mod grpc;
pub mod member;
pub mod pipeline;
pub mod refs;
pub mod room;

use std::{convert::TryFrom as _, fs::File, io::Read as _, path::Path};

use actix::Addr;
use derive_more::Display;
use failure::{Error, Fail};
use medea_client_api_proto::{MemberId, RoomId};
use serde::Deserialize;

use crate::{
    api::control::callback::url::CallbackUrlParseError,
    log::prelude::*,
    signalling::room_service::{
        RoomService, RoomServiceError, StartStaticRooms,
    },
};

use self::{pipeline::Pipeline, refs::src_uri::SrcParseError};

#[doc(inline)]
pub use self::{
    endpoints::{
        webrtc_play_endpoint::WebRtcPlayId,
        webrtc_publish_endpoint::WebRtcPublishId, EndpointSpec,
        Id as EndpointId,
    },
    member::MemberSpec,
    room::{RoomElement, RoomSpec},
};

/// Errors which may occur while deserializing protobuf spec.
#[derive(Debug, Fail, Display)]
pub enum TryFromProtobufError {
    /// Error while parsing [`SrcUri`] of [`WebRtcPlayEndpoint`].
    ///
    /// [`WebRtcPlayEndpoint`]:
    /// crate::api::control::endpoints::WebRtcPlayEndpoint
    /// [`SrcUri`]: crate::api::control::refs::SrcUri
    #[display(fmt = "Src uri parse error: {:?}", _0)]
    SrcUriError(SrcParseError),

    /// `Room` element doesn't have `Member` element. Currently this is
    /// unimplemented.
    #[display(
        fmt = "Room element [id = {}] doesn't have Member element. Currently \
               this is unimplemented.",
        _0
    )]
    NotMemberElementInRoomElement(String),

    /// `Room` element doesn't have `Member` element. Currently this is
    /// unimplemented.
    #[display(fmt = "Expected element of type [{}]. Id [{}]", _0, _1)]
    ExpectedOtherElement(String, String),

    /// Element is `None`, but expected `Some`.
    #[display(fmt = "Element is None, expected Some. Id [{}]", _0)]
    EmptyElement(String),

    /// Provided `Endpoint` is unimplemented.
    #[display(fmt = "Endpoint is unimplemented. Id [{}]", _0)]
    UnimplementedEndpoint(String),

    /// Error while [`CallbackUrl`] parsing.
    ///
    /// [`CallbackUrl`]: crate::api::control::callback::CallbackUrl
    #[display(fmt = "Error while parsing callback URL. {:?}", _0)]
    CallbackUrlParseErr(CallbackUrlParseError),

    /// Some element from a spec contains negative [`Duration`], but it's not
    /// supported.
    ///
    /// [`Duration`]: std::time::Duration
    #[display(
        fmt = "Element [id = {}] contains negative duration field `{}`",
        _0,
        _1
    )]
    NegativeDuration(String, &'static str),
}

impl From<SrcParseError> for TryFromProtobufError {
    fn from(from: SrcParseError) -> Self {
        Self::SrcUriError(from)
    }
}

impl From<CallbackUrlParseError> for TryFromProtobufError {
    fn from(from: CallbackUrlParseError) -> Self {
        Self::CallbackUrlParseErr(from)
    }
}

/// Root elements of [Control API] spec.
///
/// [Control API]: https://tinyurl.com/yxsqplq7
#[derive(Clone, Deserialize, Debug)]
#[serde(tag = "kind")]
pub enum RootElement {
    /// Represents [`RoomSpec`].
    /// Can transform into [`RoomSpec`] by `RoomSpec::try_from`.
    Room {
        id: RoomId,
        spec: Pipeline<MemberId, RoomElement>,
    },
}

/// Errors that can occur when we try transform some spec from `Element`.
/// This error used in all [`TryFrom`] of Control API.
///
/// [`TryFrom`]: std::convert::TryFrom
#[allow(clippy::pub_enum_variant_names)]
#[derive(Clone, Debug, Display, Fail)]
pub enum TryFromElementError {
    /// Element is not `Room`.
    #[display(fmt = "Element is not Room")]
    NotRoom,

    /// Element is not `Member`.
    #[display(fmt = "Element is not Member")]
    NotMember,
}

/// Errors which can happen while loading static [Control API] specs.
///
/// [Control API]: https://tinyurl.com/yxsqplq7
#[allow(clippy::pub_enum_variant_names)]
#[derive(Debug, Display, Fail)]
pub enum LoadStaticControlSpecsError {
    /// Error while reading default or provided in config
    /// (`MEDEA_CONTROL_API.STATIC_SPECS_DIR` environment variable) static
    /// [Control API] specs dir.
    ///
    /// Atm we only should print `warn!` message to log which prints that
    /// static specs not loaded.
    ///
    /// [Control API]: https://tinyurl.com/yxsqplq7
    #[display(fmt = "Error while reading static control API specs dir.")]
    SpecDirReadError(std::io::Error),

    /// I/O error while reading static [Control API] specs.
    ///
    /// [Control API]: https://tinyurl.com/yxsqplq7
    #[display(fmt = "I/O error while reading specs. {:?}", _0)]
    IoError(std::io::Error),

    /// Conflict in static [Control API] specs.
    ///
    /// [Control API]: https://tinyurl.com/yxsqplq7
    #[display(
        fmt = "Try from element error while loading static specs. {:?}",
        _0
    )]
    TryFromElementError(TryFromElementError),

    /// Error while deserialization static [Control API] specs from YAML file.
    ///
    /// [Control API]: https://tinyurl.com/yxsqplq7
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

/// Loads [`RoomSpec`] from file with YAML format.
///
/// # Errors
///
/// Errors with [`LoadStaticControlSpecsError::IoError`] if reading of provided
/// [`Path`] to file fails.
///
/// Errors with [`LoadStaticControlSpecsError::YamlDeserializationError`] if
/// YAML deserialization fails.
///
/// Errors with [`LoadStaticControlSpecsError::TryFromElementError`] if
/// [`RoomSpec`] conversation fails.
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

/// Loads all [`RoomSpec`] from YAML files from provided path.
///
/// # Errors
///
/// Errors with [`LoadStaticControlSpecsError::SpecDirReadError`] if reading
/// provided [`Path`] fails.
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

/// Starts all [`Room`]s from static [Control API] specs.
///
/// # Errors
///
/// Errors if unable to send message to [`RoomService`] actor.
///
/// # Panics
///
/// If the [`RoomService`] fails to start all static [`Room`]s.
///
/// [Control API]: https://tinyurl.com/yxsqplq7
/// [`Room`]: crate::signalling::room::Room
pub async fn start_static_rooms(
    room_service: &Addr<RoomService>,
) -> Result<(), Error> {
    match room_service.send(StartStaticRooms).await? {
        Err(RoomServiceError::FailedToLoadStaticSpecs(
            LoadStaticControlSpecsError::SpecDirReadError(e),
        )) => warn!(
            "Error while reading static control API specs dir. Control API \
             specs not loaded. {}",
            e,
        ),
        Err(e) => panic!("{}", e),
        Ok(_) => {}
    };
    Ok(())
}

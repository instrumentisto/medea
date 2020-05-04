//! Control API callbacks implementation.

pub mod clients;
pub mod service;
pub mod url;

use std::convert::From;

use actix::Message;
use chrono::{DateTime, Utc};
use clients::CallbackClientError;
use derive_more::{Display, From};
use medea_control_api_proto::grpc::callback as proto;

use crate::{
    api::control::refs::StatefulFid, signalling::peers::TrackMediaType,
};

/// Event for `on_leave` `Member` callback.
#[derive(Debug)]
pub struct OnLeaveEvent {
    /// Reason of why `Member` was lost.
    reason: OnLeaveReason,
}

impl OnLeaveEvent {
    #[inline]
    pub fn new(reason: OnLeaveReason) -> Self {
        Self { reason }
    }
}

impl Into<proto::OnLeave> for OnLeaveEvent {
    fn into(self) -> proto::OnLeave {
        let on_leave: proto::on_leave::Reason = self.reason.into();
        proto::OnLeave {
            reason: on_leave as i32,
        }
    }
}

/// Reason of why `Member` was lost.
#[derive(Debug)]
pub enum OnLeaveReason {
    /// `Member` was normally disconnected.
    Disconnected,

    /// Connection with `Member` was lost.
    LostConnection,

    /// Server is shutting down.
    ServerShutdown,
}

impl Into<proto::on_leave::Reason> for OnLeaveReason {
    fn into(self) -> proto::on_leave::Reason {
        match self {
            Self::LostConnection => proto::on_leave::Reason::LostConnection,
            Self::ServerShutdown => proto::on_leave::Reason::ServerShutdown,
            Self::Disconnected => proto::on_leave::Reason::Disconnected,
        }
    }
}

/// `on_join` `Member` callback for Control API.
#[derive(Debug)]
pub struct OnJoinEvent;

impl Into<proto::OnJoin> for OnJoinEvent {
    fn into(self) -> proto::OnJoin {
        proto::OnJoin {}
    }
}

/// `on_start` Control API callback.
#[derive(Debug)]
pub struct OnStartEvent {
    /// [`MediaType`] of the traffic which starts flowing in some `Endpoint`.
    pub media_type: MediaType,

    /// [`MediaDirection`] of the `Endpoint` for which this callback was
    /// received.
    pub direction: MediaDirection,
}

impl Into<proto::OnStart> for OnStartEvent {
    fn into(self) -> proto::OnStart {
        let media_type: proto::MediaType = self.media_type.into();
        let direction: proto::MediaDirection = self.direction.into();

        proto::OnStart {
            media_type: media_type as i32,
            media_direction: direction as i32,
        }
    }
}

/// Media type of the traffic which starts/stops flowing in some `Endpoint`.
///
/// This enum is used in the [`MuteState`] of the `Endpoint`s. Because of it,
/// this structure is bitflag enum.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Display)]
pub enum MediaType {
    /// Started/stopped audio traffic.
    Audio = 0b1,

    /// Started/stopped video traffic.
    Video = 0b10,

    /// Started/stopped audio and video traffic.
    ///
    /// In bitflag representation this variant will be equal to
    /// [`MediaType::Audio`] + [`MediaType::Video`].
    Both = 0b11,
}

impl Into<proto::MediaType> for MediaType {
    fn into(self) -> proto::MediaType {
        match self {
            MediaType::Audio => proto::MediaType::Audio,
            MediaType::Video => proto::MediaType::Video,
            MediaType::Both => proto::MediaType::Both,
        }
    }
}

impl From<&medea_client_api_proto::MediaType> for MediaType {
    fn from(media_type: &medea_client_api_proto::MediaType) -> Self {
        use medea_client_api_proto::MediaType as MediaTypeProto;

        match media_type {
            MediaTypeProto::Audio(_) => MediaType::Audio,
            MediaTypeProto::Video(_) => MediaType::Video,
        }
    }
}

impl From<TrackMediaType> for MediaType {
    fn from(from: TrackMediaType) -> Self {
        match from {
            TrackMediaType::Audio => Self::Audio,
            TrackMediaType::Video => Self::Video,
        }
    }
}

/// Media direction of the `Endpoint` for which `on_start` or `on_stop` Control
/// API callback was received.
#[derive(Clone, Copy, Debug)]
pub enum MediaDirection {
    /// `Endpoint` is a publisher.
    Publish,

    /// `Endpoint` is a player.
    Play,
}

impl Into<proto::MediaDirection> for MediaDirection {
    fn into(self) -> proto::MediaDirection {
        match self {
            MediaDirection::Play => proto::MediaDirection::Play,
            MediaDirection::Publish => proto::MediaDirection::Publish,
        }
    }
}

/// Reason of why some `Endpoint` was stopped.
#[derive(Clone, Copy, Debug)]
pub enum OnStopReason {
    /// All traffic of some `Endpoint` was stopped flowing.
    TrafficNotFlowing,

    /// `Endpoint` was muted.
    Muted,

    /// Source `Endpoint` of a `Endpoint` for which received this `on_stop`
    /// callback was muted.
    SrcMuted,

    /// Some traffic flows within `Endpoint`, but incorrectly.
    WrongTrafficFlowing,
}

impl Into<proto::on_stop::Reason> for OnStopReason {
    fn into(self) -> proto::on_stop::Reason {
        match self {
            OnStopReason::TrafficNotFlowing => {
                proto::on_stop::Reason::TrafficNotFlowing
            }
            OnStopReason::Muted => proto::on_stop::Reason::Muted,
            OnStopReason::SrcMuted => proto::on_stop::Reason::SrcMuted,
            OnStopReason::WrongTrafficFlowing => {
                proto::on_stop::Reason::WrongTrafficFlowing
            }
        }
    }
}

/// `on_stop` Control API callback.
#[derive(Debug)]
pub struct OnStopEvent {
    /// [`MediaType`] of the traffic which stops flowing in some `Endpoint`.
    pub media_type: MediaType,

    /// [`MediaDirection`] of the `Endpoint` for which this callback was
    /// received.
    pub media_direction: MediaDirection,

    /// Reason of why `Endpoint` was stopped.
    pub reason: OnStopReason,
}

impl Into<proto::OnStop> for OnStopEvent {
    fn into(self) -> proto::OnStop {
        let media_type: proto::MediaType = self.media_type.into();
        let media_direction: proto::MediaDirection =
            self.media_direction.into();
        let reason: proto::on_stop::Reason = self.reason.into();

        proto::OnStop {
            media_type: media_type as i32,
            media_direction: media_direction as i32,
            reason: reason as i32,
        }
    }
}

/// All possible Control API callbacks.
#[derive(Debug, From)]
pub enum CallbackEvent {
    Join(OnJoinEvent),
    Leave(OnLeaveEvent),
    Start(OnStartEvent),
    Stop(OnStopEvent),
}

impl Into<proto::request::Event> for CallbackEvent {
    fn into(self) -> proto::request::Event {
        match self {
            Self::Join(on_join) => {
                proto::request::Event::OnJoin(on_join.into())
            }
            Self::Leave(on_leave) => {
                proto::request::Event::OnLeave(on_leave.into())
            }
            Self::Start(on_start) => {
                proto::request::Event::OnStart(on_start.into())
            }
            Self::Stop(on_stop) => {
                proto::request::Event::OnStop(on_stop.into())
            }
        }
    }
}

/// Control API callback.
///
/// Used for sending callbacks with [`CallbackClient::send`].
///
/// [`CallbackClient::send`]:
/// crate::api::control::callback::clients::CallbackClient::send
#[derive(Debug, Message)]
#[rtype(result = "Result<(), CallbackClientError>")]
pub struct CallbackRequest {
    /// FID (Full ID) of element with which event was occurred.
    fid: StatefulFid,

    /// [`CallbackEvent`] which occurred.
    event: CallbackEvent,

    /// Time at which event occurred.
    at: DateTime<Utc>,
}

impl CallbackRequest {
    /// Returns new [`CallbackRequest`] with provided fields.
    pub fn new<F, E, D>(fid: F, event: E, datetime: D) -> Self
    where
        E: Into<CallbackEvent>,
        F: Into<StatefulFid>,
        D: Into<DateTime<Utc>>,
    {
        Self {
            fid: fid.into(),
            event: event.into(),
            at: datetime.into(),
        }
    }

    /// Returns new [`CallbackRequest`] with provided fields and `at` field set
    /// to current date time.
    pub fn new_at_now<F, E>(fid: F, event: E) -> Self
    where
        E: Into<CallbackEvent>,
        F: Into<StatefulFid>,
    {
        Self {
            fid: fid.into(),
            event: event.into(),
            at: Utc::now(),
        }
    }
}

impl Into<proto::Request> for CallbackRequest {
    fn into(self) -> proto::Request {
        proto::Request {
            event: Some(self.event.into()),
            fid: self.fid.to_string(),
            at: self.at.to_rfc3339(),
        }
    }
}

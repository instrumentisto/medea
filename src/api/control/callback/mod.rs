//! Control API callbacks implementation.

pub mod clients;
pub mod service;
pub mod url;

use actix::Message;
use chrono::{DateTime, Utc};
use clients::CallbackClientError;
use derive_more::{Display, From};
use medea_control_api_proto::grpc::callback as proto;

use crate::api::control::refs::StatefulFid;

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

/// All callbacks which can happen.
#[derive(Debug, From)]
pub enum CallbackEvent {
    OnJoin(OnJoinEvent),
    OnLeave(OnLeaveEvent),
}

impl Into<proto::request::Event> for CallbackEvent {
    fn into(self) -> proto::request::Event {
        match self {
            Self::OnJoin(on_join) => {
                proto::request::Event::OnJoin(on_join.into())
            }
            Self::OnLeave(on_leave) => {
                proto::request::Event::OnLeave(on_leave.into())
            }
        }
    }
}

/// Media type of the traffic which starts/stops flowing in some `Endpoint`.
///
/// This enum is used in [`MuteState`] of `Endpoint`s. That's why it represents
/// a bitflag enum.
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

impl MediaType {
    /// Returns [`MediaType`] which was started based on the provided
    /// [`MediaType`]s.
    ///
    /// This [`MediaType`] should be what was before `RTCStat` update and
    /// as argument is [`MediaType`] which was got after `RTCStat` update.
    pub fn get_started(self, after: Self) -> Option<Self> {
        match self {
            MediaType::Audio => match after {
                MediaType::Video => Some(MediaType::Audio),
                _ => None,
            },
            MediaType::Video => match after {
                MediaType::Audio => Some(MediaType::Video),
                _ => None,
            },
            MediaType::Both => match after {
                MediaType::Audio => Some(MediaType::Video),
                MediaType::Video => Some(MediaType::Audio),
                _ => None,
            },
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

/// Media direction of the `Endpoint` for which `on_start` or `on_stop` Control
/// API callback was received.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum MediaDirection {
    /// `Endpoint` is a publisher.
    Publish,

    /// `Endpoint` is a player.
    Play,
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
    /// Returns [`CallbackRequest`] with provided fields and current time as
    /// `at`.
    pub fn new(element: StatefulFid, event: CallbackEvent) -> Self {
        Self {
            fid: element,
            event,
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

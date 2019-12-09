//! Control API callbacks implementation.

pub mod clients;
pub mod service;
pub mod url;

use chrono::{DateTime, Utc};
use derive_more::From;
use medea_control_api_proto::grpc::callback::{
    OnJoin as OnJoinProto, OnJoin, OnLeave as OnLeaveProto,
    OnLeave_Reason as OnLeaveReasonProto, Request as CallbackRequestProto,
    Request_oneof_event as RequestOneofEventProto,
};

use crate::api::control::refs::StatefulFid;

/// Event for `on_leave` `Member` callback.
#[derive(Debug)]
pub struct OnLeaveEvent {
    /// Reason of why `Member` was lost.
    reason: OnLeaveReason,
}

impl OnLeaveEvent {
    pub fn new(reason: OnLeaveReason) -> Self {
        Self { reason }
    }
}

impl Into<OnLeaveProto> for OnLeaveEvent {
    fn into(self) -> OnLeaveProto {
        let mut proto = OnLeaveProto::new();
        proto.set_reason(self.reason.into());
        proto
    }
}

/// Reason of why `Member` was lost.
#[derive(Debug)]
pub enum OnLeaveReason {
    /// Server is shutting down.
    ServerShutdown,

    /// Connection with `Member` was lost.
    LostConnection,

    /// Member was normally disconnected.
    Disconnected,
}

impl Into<OnLeaveReasonProto> for OnLeaveReason {
    fn into(self) -> OnLeaveReasonProto {
        match self {
            Self::LostConnection => OnLeaveReasonProto::LOST_CONNECTION,
            Self::ServerShutdown => OnLeaveReasonProto::SERVER_SHUTDOWN,
            Self::Disconnected => OnLeaveReasonProto::DISCONNECTED,
        }
    }
}

/// `on_join` `Member` callback for Control API.
#[derive(Debug)]
pub struct OnJoinEvent;

impl Into<OnJoinProto> for OnJoinEvent {
    fn into(self) -> OnJoin {
        OnJoinProto::new()
    }
}

/// All callbacks which can happen.
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, From)]
pub enum CallbackEvent {
    OnJoin(OnJoinEvent),
    OnLeave(OnLeaveEvent),
}

impl Into<RequestOneofEventProto> for CallbackEvent {
    fn into(self) -> RequestOneofEventProto {
        match self {
            Self::OnJoin(on_join) => {
                RequestOneofEventProto::on_join(on_join.into())
            }
            Self::OnLeave(on_leave) => {
                RequestOneofEventProto::on_leave(on_leave.into())
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
#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
pub struct CallbackRequest {
    /// FID (Full ID) of element with which event was occurred.
    fid: StatefulFid,

    /// [`CallbackEvent`] which occured.
    event: CallbackEvent,

    /// Time on which event was occurred.
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

impl Into<CallbackRequestProto> for CallbackRequest {
    fn into(self) -> CallbackRequestProto {
        let mut proto = CallbackRequestProto::new();
        proto.event = Some(self.event.into());
        proto.set_fid(self.fid.to_string());
        proto.set_at(self.at.to_rfc3339());
        proto
    }
}

//! Control API callbacks implementation.

pub mod repo;
pub mod services;
pub mod url;

use actix::Message;
use chrono::{DateTime, Utc};
use derive_more::From;
use medea_control_api_proto::grpc::callback::{
    OnJoin as OnJoinProto, OnJoin, OnLeave as OnLeaveProto,
    OnLeave_Reason as OnLeaveReasonProto, Request,
    Request_oneof_event as RequestOneofEventProto,
};

use crate::api::control::refs::StatefulFid;

/// Event for `on_leave` `Member` callback.
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
pub enum OnLeaveReason {
    /// Server is shutting down.
    ServerShutdown,

    /// Connection with `Member` was lost.
    LostConnection,
}

impl Into<OnLeaveReasonProto> for OnLeaveReason {
    fn into(self) -> OnLeaveReasonProto {
        match self {
            Self::LostConnection => OnLeaveReasonProto::LOST_CONNECTION,
            Self::ServerShutdown => OnLeaveReasonProto::SERVER_SHUTDOWN,
        }
    }
}

/// `on_join` `Member` callback for Control API.
pub struct OnJoinEvent;

impl Into<OnJoinProto> for OnJoinEvent {
    fn into(self) -> OnJoin {
        OnJoinProto::new()
    }
}

/// All callbacks which can happen.
#[allow(clippy::module_name_repetitions)]
#[derive(From)]
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
/// This struct is used as [`Message`] for sending callback in all callback
/// services.
#[derive(Message)]
#[rtype(result = "Result<(), ()>")]
pub struct Callback {
    element: StatefulFid,
    event: CallbackEvent,
    at: DateTime<Utc>,
}

impl Callback {
    /// Returns [`Callback`] with provided fields and current time as `at`.
    pub fn new(element: StatefulFid, event: CallbackEvent) -> Self {
        Self {
            element,
            event,
            at: chrono::Utc::now(),
        }
    }
}

impl Into<Request> for Callback {
    fn into(self) -> Request {
        let mut proto = Request::new();
        proto.event = Some(self.event.into());
        proto.set_element(self.element.to_string());
        proto.set_at(self.at.to_rfc3339());
        proto
    }
}

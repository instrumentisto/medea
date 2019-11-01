pub mod callback_repo;
pub mod callback_url;
pub mod grpc_callback_service;

use actix::Message;
use chrono::{DateTime, Utc};
use derive_more::From;
use medea_control_api_proto::grpc::callback::{
    OnJoin as OnJoinProto, OnJoin, OnLeave as OnLeaveProto, OnLeave,
    OnLeave_Reason as OnLeaveReasonProto, OnLeave_Reason, Request,
    Request_oneof_event as RequestOneofEventProto,
};

use crate::api::control::refs::StatefulFid;

pub struct OnLeaveEvent {
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

pub enum OnLeaveReason {
    ServerShutdown,
    LostConnection,
}

impl Into<OnLeaveReasonProto> for OnLeaveReason {
    fn into(self) -> OnLeaveReasonProto {
        match self {
            OnLeaveReason::LostConnection => {
                OnLeaveReasonProto::LOST_CONNECTION
            }
            OnLeaveReason::ServerShutdown => {
                OnLeaveReasonProto::SERVER_SHUTDOWN
            }
        }
    }
}

pub struct OnJoinEvent;

impl Into<OnJoinProto> for OnJoinEvent {
    fn into(self) -> OnJoin {
        OnJoinProto::new()
    }
}

#[derive(From)]
pub enum CallbackEvent {
    OnJoin(OnJoinEvent),
    OnLeave(OnLeaveEvent),
}

impl Into<RequestOneofEventProto> for CallbackEvent {
    fn into(self) -> RequestOneofEventProto {
        match self {
            CallbackEvent::OnJoin(on_join) => {
                RequestOneofEventProto::on_join(on_join.into())
            }
            CallbackEvent::OnLeave(on_leave) => {
                RequestOneofEventProto::on_leave(on_leave.into())
            }
        }
    }
}

#[derive(Message)]
#[rtype(result = "Result<(), ()>")]
pub struct Callback {
    element: StatefulFid,
    event: CallbackEvent,
    at: DateTime<Utc>,
}

impl Callback {
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

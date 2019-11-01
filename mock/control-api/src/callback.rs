use medea_control_api_proto::grpc::callback::{
    OnJoin as OnJoinProto, OnLeave as OnLeaveProto,
    OnLeave_Reason as OnLeaveReasonProto, Request as CallbackProto,
    Request_oneof_event as CallbackEventProto, Request_oneof_event,
};
use serde::Serialize;

#[derive(Clone, Serialize)]
pub struct OnLeave {
    reason: OnLeaveReason,
}

impl From<OnLeaveProto> for OnLeave {
    fn from(proto: OnLeaveProto) -> Self {
        Self {
            reason: proto.get_reason().into(),
        }
    }
}

#[derive(Clone, Serialize)]
pub enum OnLeaveReason {
    ServerShutdown,
    LostConnection,
}

impl From<OnLeaveReasonProto> for OnLeaveReason {
    fn from(proto: OnLeaveReasonProto) -> Self {
        match proto {
            OnLeaveReasonProto::SERVER_SHUTDOWN => {
                OnLeaveReason::ServerShutdown
            }
            OnLeaveReasonProto::LOST_CONNECTION => {
                OnLeaveReason::LostConnection
            }
        }
    }
}

#[derive(Clone, Serialize)]
pub struct OnJoin;

impl From<OnJoinProto> for OnJoin {
    fn from(proto: OnJoinProto) -> Self {
        Self
    }
}

#[derive(Clone, Serialize)]
#[serde(tag = "type")]
pub enum CallbackEvent {
    OnJoin(OnJoin),
    OnLeave(OnLeave),
}

impl From<CallbackEventProto> for CallbackEvent {
    fn from(proto: CallbackEventProto) -> Self {
        match proto {
            CallbackEventProto::on_leave(on_leave) => {
                Self::OnLeave(on_leave.into())
            }
            CallbackEventProto::on_join(on_join) => {
                Self::OnJoin(on_join.into())
            }
        }
    }
}

#[derive(Clone, Serialize)]
pub struct Callback {
    element: String,
    event: CallbackEvent,
    at: String,
}

impl From<CallbackProto> for Callback {
    fn from(mut proto: CallbackProto) -> Self {
        Self {
            element: proto.take_element(),
            at: proto.take_at(),
            event: proto.event.unwrap().into(),
        }
    }
}

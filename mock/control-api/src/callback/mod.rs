//! Control API callback implementation.

pub mod server;

use medea_control_api_proto::grpc::callback::{
    OnJoin as OnJoinProto, OnLeave as OnLeaveProto,
    OnLeave_Reason as OnLeaveReasonProto, Request as CallbackProto,
    Request_oneof_event as CallbackEventProto,
};
use serde::Serialize;

/// `OnLeave` callback of Control API.
#[derive(Clone, Serialize)]
pub struct OnLeave {
    /// Reason of why `Member` leaves.
    reason: OnLeaveReason,
}

impl From<OnLeaveProto> for OnLeave {
    fn from(proto: OnLeaveProto) -> Self {
        Self {
            reason: proto.get_reason().into(),
        }
    }
}

/// Reason of why `Member` leaves.
#[derive(Clone, Serialize)]
pub enum OnLeaveReason {
    /// Server is shutting down.
    ServerShutdown,

    /// Connection with `Member` was lost.
    LostConnection,

    /// Member was normally disconnected.
    Disconnected,

    /// Member was evicted from the Medea media server by Control API.
    Evicted,
}

impl From<OnLeaveReasonProto> for OnLeaveReason {
    fn from(proto: OnLeaveReasonProto) -> Self {
        match proto {
            OnLeaveReasonProto::SERVER_SHUTDOWN => Self::ServerShutdown,
            OnLeaveReasonProto::LOST_CONNECTION => Self::LostConnection,
            OnLeaveReasonProto::EVICTED => Self::Evicted,
            OnLeaveReasonProto::DISCONNECTED => Self::Disconnected,
        }
    }
}

/// `OnJoin` callback for Control API.
#[derive(Clone, Serialize)]
pub struct OnJoin;

impl From<OnJoinProto> for OnJoin {
    fn from(_: OnJoinProto) -> Self {
        Self
    }
}

/// All callbacks which can happen.
#[allow(clippy::module_name_repetitions)]
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

/// Control API callback.
#[derive(Clone, Serialize)]
pub struct Callback {
    /// FID (Full ID) of element with which this event was occurred.
    fid: String,

    /// Event which occurred.
    event: CallbackEvent,

    /// Time on which callback was occurred.
    at: String,
}

impl From<CallbackProto> for Callback {
    fn from(mut proto: CallbackProto) -> Self {
        Self {
            fid: proto.take_fid(),
            at: proto.take_at(),
            event: proto.event.unwrap().into(),
        }
    }
}

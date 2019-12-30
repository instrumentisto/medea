//! Control API Callback service implementation.

pub mod server;

use medea_control_api_proto::grpc::callback::{
    Request as CallbackProto, Request_oneof_event as CallbackEventProto,
};
use serde::Serialize;

/// All callbacks which can happen.
#[derive(Clone, Serialize)]
#[serde(tag = "type")]
pub enum CallbackEvent {
    OnJoin(join::OnJoin),
    OnLeave(leave::OnLeave),
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
pub struct CallbackItem {
    /// FID (Full ID) of element with which this event was occurred.
    fid: String,

    /// Event which occurred.
    event: CallbackEvent,

    /// Time on which callback was occurred.
    at: String,
}

impl From<CallbackProto> for CallbackItem {
    fn from(mut proto: CallbackProto) -> Self {
        Self {
            fid: proto.take_fid(),
            at: proto.take_at(),
            event: proto.event.unwrap().into(),
        }
    }
}

/// `on_join` callback's related entities and implementations.
mod join {
    use medea_control_api_proto::grpc::callback::OnJoin as OnJoinProto;
    use serde::Serialize;

    /// `OnJoin` callback for Control API.
    #[derive(Clone, Serialize)]
    pub struct OnJoin;

    impl From<OnJoinProto> for OnJoin {
        fn from(_: OnJoinProto) -> Self {
            Self
        }
    }
}

/// `on_leave` callback's related entities and implementations.
mod leave {
    use medea_control_api_proto::grpc::callback::{
        OnLeave as OnLeaveProto, OnLeave_Reason as OnLeaveReasonProto,
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
        /// `Member` was normally disconnected.
        Disconnected,

        /// Connection with `Member` was lost.
        LostConnection,

        /// Server is shutting down.
        ServerShutdown,
    }

    impl From<OnLeaveReasonProto> for OnLeaveReason {
        fn from(proto: OnLeaveReasonProto) -> Self {
            match proto {
                OnLeaveReasonProto::SERVER_SHUTDOWN => Self::ServerShutdown,
                OnLeaveReasonProto::LOST_CONNECTION => Self::LostConnection,
                OnLeaveReasonProto::DISCONNECTED => Self::Disconnected,
            }
        }
    }
}

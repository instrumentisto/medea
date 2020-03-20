//! Control API Callback service implementation.

pub mod server;

use medea_control_api_proto::grpc::callback as proto;
use serde::Serialize;

/// All callbacks which can happen.
#[derive(Clone, Serialize)]
#[serde(tag = "type")]
pub enum CallbackEvent {
    Join(join::OnJoin),
    Leave(leave::OnLeave),
    Start(on_start::OnStart),
    Stop(on_stop::OnStop),
}

impl From<proto::request::Event> for CallbackEvent {
    fn from(proto: proto::request::Event) -> Self {
        match proto {
            proto::request::Event::OnLeave(on_leave) => {
                Self::Leave(on_leave.into())
            }
            proto::request::Event::OnJoin(on_join) => {
                Self::Join(on_join.into())
            }
            proto::request::Event::OnStart(on_start) => {
                Self::Start(on_start.into())
            }
            proto::request::Event::OnStop(on_stop) => {
                Self::Stop(on_stop.into())
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

impl From<proto::Request> for CallbackItem {
    fn from(proto: proto::Request) -> Self {
        Self {
            fid: proto.fid,
            at: proto.at,
            event: proto.event.unwrap().into(),
        }
    }
}

/// `on_join` callback's related entities and implementations.
mod join {
    use medea_control_api_proto::grpc::callback as proto;
    use serde::Serialize;

    /// `OnJoin` callback for Control API.
    #[derive(Clone, Serialize)]
    pub struct OnJoin;

    impl From<proto::OnJoin> for OnJoin {
        fn from(_: proto::OnJoin) -> Self {
            Self
        }
    }
}

/// `on_start` callback's related entities and implementations.
mod on_start {
    use super::*;

    /// `OnStart` callback of Control API.
    #[derive(Clone, Serialize)]
    pub struct OnStart;

    impl From<proto::OnStart> for OnStart {
        fn from(_: proto::OnStart) -> Self {
            Self
        }
    }
}

/// `on_stop` callback's related entities and implementations.
mod on_stop {
    use super::*;

    /// `OnStop` callback of Control API.
    #[derive(Clone, Serialize)]
    pub struct OnStop;

    impl From<proto::OnStop> for OnStop {
        fn from(_: proto::OnStop) -> Self {
            Self
        }
    }
}

/// `on_leave` callback's related entities and implementations.
mod leave {
    use medea_control_api_proto::grpc::callback as proto;
    use serde::Serialize;

    /// `OnLeave` callback of Control API.
    #[derive(Clone, Serialize)]
    pub struct OnLeave {
        /// Reason of why `Member` leaves.
        reason: OnLeaveReason,
    }

    impl From<proto::OnLeave> for OnLeave {
        fn from(proto: proto::OnLeave) -> Self {
            Self {
                reason: proto::on_leave::Reason::from_i32(proto.reason)
                    .unwrap_or_default()
                    .into(),
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

    impl From<proto::on_leave::Reason> for OnLeaveReason {
        fn from(proto: proto::on_leave::Reason) -> Self {
            use proto::on_leave::Reason::*;
            match proto {
                ServerShutdown => Self::ServerShutdown,
                LostConnection => Self::LostConnection,
                Disconnected => Self::Disconnected,
            }
        }
    }
}

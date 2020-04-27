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

/// Media direction of the `Endpoint` for which `on_start` or `on_stop` Control
/// API callback was received.
#[derive(Clone, Serialize)]
enum MediaDirection {
    /// `Endpoint` is a publisher.
    Publish,

    /// `Endpoint` is a player.
    Play,
}

impl From<proto::MediaDirection> for MediaDirection {
    fn from(proto: proto::MediaDirection) -> Self {
        use proto::MediaDirection::*;

        match proto {
            Publish => Self::Publish,
            Play => Self::Play,
        }
    }
}

/// Media type of the traffic which starts/stops flowing in some `Endpoint`.
#[derive(Clone, Serialize)]
enum MediaType {
    /// Started/stopped video traffic.
    Video,

    /// Started/stopped audio traffic.
    Audio,

    /// Started/stopped audio and video traffic.
    Both,
}

impl From<proto::MediaType> for MediaType {
    fn from(proto: proto::MediaType) -> Self {
        use proto::MediaType::*;
        match proto {
            Audio => Self::Audio,
            Video => Self::Video,
            Both => Self::Both,
        }
    }
}

/// `on_start` callback's related entities and implementations.
mod on_start {
    use super::{proto, MediaDirection, MediaType, Serialize};

    /// `OnStart` callback of Control API.
    #[derive(Clone, Serialize)]
    pub struct OnStart {
        /// [`MediaDirection`] of the `Endpoint` for which this callback was
        /// received.
        media_direction: MediaDirection,

        /// [`MediaType`] of the traffic which starts flowing in some
        /// `Endpoint`.
        media_type: MediaType,
    }

    impl From<proto::OnStart> for OnStart {
        fn from(proto: proto::OnStart) -> Self {
            Self {
                media_direction: proto::MediaDirection::from_i32(
                    proto.media_direction,
                )
                .unwrap_or_default()
                .into(),
                media_type: proto::MediaType::from_i32(proto.media_type)
                    .unwrap_or_default()
                    .into(),
            }
        }
    }
}

/// `on_stop` callback's related entities and implementations.
mod on_stop {
    use super::{proto, MediaDirection, MediaType, Serialize};

    /// Reason of why some `Endpoint` was stopped.
    #[derive(Clone, Serialize)]
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

    impl From<proto::on_stop::Reason> for OnStopReason {
        fn from(proto: proto::on_stop::Reason) -> Self {
            use proto::on_stop::Reason::*;
            match proto {
                TrafficNotFlowing => Self::TrafficNotFlowing,
                Muted => Self::Muted,
                SrcMuted => Self::SrcMuted,
                WrongTrafficFlowing => Self::WrongTrafficFlowing,
            }
        }
    }

    /// `OnStop` callback of Control API.
    #[derive(Clone, Serialize)]
    pub struct OnStop {
        /// [`MediaType`] of the traffic which stops flowing in some
        /// `Endpoint`.
        media_type: MediaType,

        /// [`MediaDirection`] of the `Endpoint` for which this callback was
        /// received.
        media_direction: MediaDirection,

        /// Reason of why `Endpoint` was stopped.
        reason: OnStopReason,
    }

    impl From<proto::OnStop> for OnStop {
        fn from(proto: proto::OnStop) -> Self {
            Self {
                reason: proto::on_stop::Reason::from_i32(proto.reason)
                    .unwrap_or_default()
                    .into(),
                media_type: proto::MediaType::from_i32(proto.media_type)
                    .unwrap_or_default()
                    .into(),
                media_direction: proto::MediaDirection::from_i32(
                    proto.media_direction,
                )
                .unwrap_or_default()
                .into(),
            }
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

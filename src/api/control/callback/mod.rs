//! Control API callbacks implementation.

pub mod clients;
pub mod service;
pub mod url;

use chrono::{DateTime, Utc};
use derive_more::From;
use medea_control_api_proto::grpc::medea_callback::{
    on_leave::Reason as OnLeaveReasonProto,
    request::Event as RequestOneofEventProto, OnJoin as OnJoinProto, OnJoin,
    OnLeave as OnLeaveProto, Request as CallbackRequestProto,
};

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

impl Into<OnLeaveProto> for OnLeaveEvent {
    fn into(self) -> OnLeaveProto {
        let on_leave: OnLeaveReasonProto = self.reason.into();
        OnLeaveProto {
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

impl Into<OnLeaveReasonProto> for OnLeaveReason {
    fn into(self) -> OnLeaveReasonProto {
        match self {
            Self::LostConnection => OnLeaveReasonProto::LostConnection,
            Self::ServerShutdown => OnLeaveReasonProto::ServerShutdown,
            Self::Disconnected => OnLeaveReasonProto::Disconnected,
        }
    }
}

/// `on_join` `Member` callback for Control API.
#[derive(Debug)]
pub struct OnJoinEvent;

impl Into<OnJoinProto> for OnJoinEvent {
    fn into(self) -> OnJoin {
        OnJoinProto {}
    }
}

/// All callbacks which can happen.
#[derive(Debug, From)]
pub enum CallbackEvent {
    OnJoin(OnJoinEvent),
    OnLeave(OnLeaveEvent),
}

impl Into<RequestOneofEventProto> for CallbackEvent {
    fn into(self) -> RequestOneofEventProto {
        match self {
            Self::OnJoin(on_join) => {
                RequestOneofEventProto::OnJoin(on_join.into())
            }
            Self::OnLeave(on_leave) => {
                RequestOneofEventProto::OnLeave(on_leave.into())
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
#[derive(Debug)]
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

impl Into<CallbackRequestProto> for CallbackRequest {
    fn into(self) -> CallbackRequestProto {
        CallbackRequestProto {
            event: Some(self.event.into()),
            fid: self.fid.to_string(),
            at: self.at.to_rfc3339(),
        }
    }
}

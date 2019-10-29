pub mod callback_repo;
pub mod callback_url;
pub mod grpc_callback_service;

use chrono::{DateTime, Utc};

use crate::api::control::refs::StatefulFid;

pub enum MemberCallbackEvent {
    OnJoin,
    OnLeave,
}

pub enum CallbackEvent {
    Member(MemberCallbackEvent),
}

pub struct Callback {
    element: StatefulFid,
    event: CallbackEvent,
    at: DateTime<Utc>,
}

pub mod callback_repo;
pub mod callback_url;
pub mod grpc_callback_service;

use actix::Message;
use chrono::{DateTime, Utc};
use medea_control_api_proto::grpc::callback::{
    Request_Event as RequestEvent, Request_Event,
};

use crate::api::control::refs::StatefulFid;

pub enum MemberCallbackEvent {
    OnJoin,
    OnLeave,
}

pub enum CallbackEvent {
    Member(MemberCallbackEvent),
}

impl Into<RequestEvent> for CallbackEvent {
    fn into(self) -> RequestEvent {
        match self {
            CallbackEvent::Member(member_event) => match member_event {
                MemberCallbackEvent::OnJoin => RequestEvent::ON_JOIN,
                MemberCallbackEvent::OnLeave => RequestEvent::ON_LEAVE,
            },
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

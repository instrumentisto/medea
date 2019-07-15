//! [`RpcConnection`] with related messages.

use std::fmt;

use actix::Message;
use futures::Future;
use macro_attr::*;
use medea_client_api_proto::{Command, Event};
use newtype_derive::NewtypeFrom;

use crate::api::control::MemberId;

macro_attr! {
    /// Wrapper [`Command`] for implements actix [`Message`].
    #[derive(Message, NewtypeFrom!)]
    #[rtype(result = "Result<(), ()>")]
    pub struct CommandMessage(Command);
}
macro_attr! {
    /// Wrapper [`Event`] for implements actix [`Message`].
    #[derive(Message, NewtypeFrom!)]
    pub struct EventMessage(Event);
}

/// Abstraction over RPC connection with some remote [`Member`].
pub trait RpcConnection: fmt::Debug + Send {
    /// Closes [`RpcConnection`].
    /// No [`RpcConnectionClosed`] signals should be emitted.
    /// Always returns success.
    fn close(&mut self) -> Box<dyn Future<Item = (), Error = ()>>;

    /// Sends [`Event`] to remote [`Member`].
    fn send_event(
        &self,
        msg: EventMessage,
    ) -> Box<dyn Future<Item = (), Error = ()>>;
}

/// Signal for authorizing new [`RpcConnection`] before establishing.
#[derive(Debug, Message)]
#[rtype(result = "Result<(), AuthorizationError>")]
pub struct Authorize {
    /// ID of [`Member`] to authorize [`RpcConnection`] for.
    pub member_id: MemberId,
    /// Credentials to authorize [`RpcConnection`] with.
    pub credentials: String, // TODO: &str when futures will allow references
}

/// Error of authorization [`RpcConnection`] in [`Room`].
#[derive(Debug)]
pub enum AuthorizationError {
    /// Authorizing [`Member`] does not exists in the [`Room`].
    MemberNotExists,
    /// Provided credentials are invalid.
    InvalidCredentials,
}

/// Signal of new [`RpcConnection`] being established with specified
/// [`Member`]. Transport should consider dropping connection if message
/// result is err.
#[derive(Debug, Message)]
#[rtype(result = "Result<(), ()>")]
#[allow(clippy::module_name_repetitions)]
pub struct RpcConnectionEstablished {
    /// ID of [`Member`] that establishes [`RpcConnection`].
    pub member_id: MemberId,
    /// Established [`RpcConnection`].
    pub connection: Box<dyn RpcConnection>,
}
/// Signal of existing [`RpcConnection`] of specified [`Member`] being
/// closed.
#[derive(Debug, Message)]
#[allow(clippy::module_name_repetitions)]
pub struct RpcConnectionClosed {
    /// ID of [`Member`] which [`RpcConnection`] is closed.
    pub member_id: MemberId,
    /// Reason of why [`RpcConnection`] is closed.
    pub reason: ClosedReason,
}

/// Reasons of why [`RpcConnection`] may be closed.
#[derive(Debug)]
pub enum ClosedReason {
    /// [`RpcConnection`] was irrevocably closed.
    Closed,
    /// [`RpcConnection`] was lost, but may be reestablished.
    Lost,
}

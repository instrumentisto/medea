//! [`RpcConnection`] with related messages.
//!
//! [`RpcConnection`]: crate::api::client::rpc_connection::RpcConnection

use std::fmt;

use actix::Message;
use derive_more::{From, Into};
use futures::future::LocalBoxFuture;
use medea_client_api_proto::{CloseDescription, Command, Event};

use crate::api::control::MemberId;

/// Newtype for [`Command`] with actix [`Message`] implementation.
#[derive(Message)]
#[rtype(result = "()")]
pub struct CommandMessage {
    pub member_id: MemberId,
    pub command: Command,
}

impl CommandMessage {
    /// Creates [`CommandMessage`].
    pub fn new(member_id: MemberId, command: Command) -> Self {
        Self { member_id, command }
    }
}

/// Newtype for [`Event`] with actix [`Message`] implementation.
#[derive(Debug, From, Into, Message)]
#[rtype(result = "()")]
pub struct EventMessage(Event);

/// Abstraction over RPC connection with some remote [`Member`].
///
/// [`Member`]: crate::signalling::elements::member::Member
pub trait RpcConnection: fmt::Debug + Send {
    /// Closes [`RpcConnection`] and sends [`CloseDescription`] to the client
    /// (in WebSocket implementation description will be sent in a [Close]
    /// frame).
    ///
    /// No [`RpcConnectionClosed`] signals should be emitted.
    ///
    /// [Close]: https://tools.ietf.org/html/rfc6455#section-5.5.1
    fn close(
        &mut self,
        close_description: CloseDescription,
    ) -> LocalBoxFuture<'static, ()>;

    /// Sends [`Event`] to remote [`Member`].
    ///
    /// [`Member`]: crate::signalling::elements::member::Member
    fn send_event(&self, msg: Event)
        -> LocalBoxFuture<'static, Result<(), ()>>;
}

/// Signal for authorizing new [`RpcConnection`] before establishing.
#[derive(Debug, Message)]
#[rtype(result = "Result<(), AuthorizationError>")]
pub struct Authorize {
    /// ID of [`Member`] to authorize [`RpcConnection`] for.
    ///
    /// [`Member`]: crate::signalling::elements::member::Member
    pub member_id: MemberId,
    /// Credentials to authorize [`RpcConnection`] with.
    pub credentials: String, // TODO: &str when futures will allow references
}

/// Error of authorization [`RpcConnection`] in [`Room`].
///
/// [`Room`]: crate::signalling::Room
#[derive(Debug)]
pub enum AuthorizationError {
    /// Authorizing [`Member`] does not exists in the [`Room`].
    ///
    /// [`Member`]: crate::signalling::elements::member::Member
    /// [`Room`]: crate::signalling::Room
    MemberNotExists,
    /// Provided credentials are invalid.
    InvalidCredentials,
}

/// Signal of new [`RpcConnection`] being established with specified [`Member`].
/// Transport should consider dropping connection if message result is err.
///
/// [`Member`]: crate::signalling::elements::member::Member
#[derive(Debug, Message)]
#[rtype(result = "Result<(), ()>")]
pub struct RpcConnectionEstablished {
    /// ID of [`Member`] that establishes [`RpcConnection`].
    ///
    /// [`Member`]: crate::signalling::elements::member::Member
    pub member_id: MemberId,
    /// Established [`RpcConnection`].
    pub connection: Box<dyn RpcConnection>,
}
/// Signal of existing [`RpcConnection`] of specified [`Member`] being closed.
///
/// [`Member`]: crate::signalling::elements::member::Member
#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct RpcConnectionClosed {
    /// ID of [`Member`] which [`RpcConnection`] is closed.
    ///
    /// [`Member`]: crate::signalling::elements::member::Member
    pub member_id: MemberId,
    /// Reason of why [`RpcConnection`] is closed.
    pub reason: ClosedReason,
}

/// Reasons of why [`RpcConnection`] may be closed.
#[derive(Debug, PartialEq)]
pub enum ClosedReason {
    /// [`RpcConnection`] was irrevocably closed.
    Closed {
        /// `true` if [`RpcConnection`] normally closed (with [`Normal`] or
        /// [`Away`] [`CloseCode`] in WebSocket implementation).
        ///
        /// `false` if [`RpcConnection`]'s closing was considered as abnormal
        /// (reconnection timeout, abnormal [`CloseCode`] etc).
        ///
        /// [`CloseCode`]: actix_http::ws::CloseCode
        /// [`Normal`]: actix_http::ws::CloseCode::Normal
        /// [`Away`]: actix_http::ws::CloseCode::Away
        normal: bool,
    },
    /// [`RpcConnection`] was lost, but may be reestablished.
    Lost,
}

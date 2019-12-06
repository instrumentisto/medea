//! [`RpcConnection`] with related messages.
//!
//! [`RpcConnection`]: crate::api::client::rpc_connection::RpcConnection

use std::fmt;

use actix::Message;
use derive_more::{From, Into};
use futures::Future;
use medea_client_api_proto::{CloseDescription, Command, Event};

use crate::api::control::MemberId;

/// Newtype for [`Command`] with actix [`Message`] implementation.
#[derive(From, Into, Message)]
#[rtype(result = "Result<(), ()>")]
pub struct CommandMessage(Command);

/// Newtype for [`Event`] with actix [`Message`] implementation.
#[derive(From, Into, Message)]
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
    /// Always succeeds.
    ///
    /// [Close]: https://tools.ietf.org/html/rfc6455#section-5.5.1
    fn close(
        &mut self,
        close_description: CloseDescription,
    ) -> Box<dyn Future<Item = (), Error = ()>>;

    /// Sends [`Event`] to remote [`Member`].
    ///
    /// [`Member`]: crate::signalling::elements::member::Member
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
#[allow(clippy::module_name_repetitions)]
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
#[allow(clippy::module_name_repetitions)]
pub struct RpcConnectionClosed {
    /// ID of [`Member`] which [`RpcConnection`] is closed.
    ///
    /// [`Member`]: crate::signalling::elements::member::Member
    pub member_id: MemberId,
    /// Reason of why [`RpcConnection`] is closed.
    pub reason: ClosedReason,
}

/// Reasons of why [`RpcConnection`] may be closed.
#[derive(Debug)]
pub enum ClosedReason {
    /// [`RpcConnection`] was irrevocably closed.
    Closed { normal: bool },
    /// [`RpcConnection`] was lost, but may be reestablished.
    Lost,
}

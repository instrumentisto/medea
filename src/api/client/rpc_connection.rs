//! [`RpcConnection`] with related messages.
//!
//! [`RpcConnection`]: crate::api::client::rpc_connection::RpcConnection

use std::{fmt, time::Duration};

use actix::Message;
use futures::future::LocalBoxFuture;
use medea_client_api_proto::{
    CloseDescription, Command, Credential, Event, MemberId, RoomId,
};

use crate::signalling::room::RoomError;

/// Newtype for [`Command`] with actix [`Message`] implementation.
#[derive(Message)]
#[rtype(result = "()")]
pub struct CommandMessage {
    /// ID of [`Member`] that sent this [`Command`] to the server.
    ///
    /// [`Member`]: crate::signalling::elements::Member
    pub member_id: MemberId,

    /// Actual [`Command`] being issued.
    pub command: Command,
}

impl CommandMessage {
    /// Creates new [`CommandMessage`].
    #[inline]
    #[must_use]
    pub fn new(member_id: MemberId, command: Command) -> Self {
        Self { member_id, command }
    }
}

/// Newtype for [`Event`] with actix [`Message`] implementation.
#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct EventMessage {
    pub room_id: RoomId,
    pub event: Event,
}

/// Abstraction over RPC connection with some remote [`Member`].
///
/// [`Member`]: crate::signalling::elements::Member
#[cfg_attr(test, mockall::automock)]
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
        room_id: RoomId,
        close_description: CloseDescription,
    ) -> LocalBoxFuture<'static, ()>;

    /// Sends [`Event`] to remote [`Member`].
    ///
    /// [`Member`]: crate::signalling::elements::Member
    fn send_event(&self, room_id: RoomId, event: Event);
}

#[cfg(test)]
impl_debug_by_struct_name!(MockRpcConnection);

/// Settings of [`RpcConnection`].
#[derive(Clone, Copy, Debug)]
pub struct RpcConnectionSettings {
    /// [`Duration`], after which [`RpcConnection`] will be considered idle if
    /// no heartbeat messages were received.
    pub idle_timeout: Duration,

    /// Interval of sending `Ping`s to remote [`Member`].
    ///
    /// [`Member`]: crate::signalling::elements::Member
    pub ping_interval: Duration,
}

/// Signal of new [`RpcConnection`] being established with specified [`Member`].
/// Transport should consider dropping connection if message result is err.
///
/// [`Member`]: crate::signalling::elements::Member
#[derive(Debug, Message)]
#[rtype(result = "Result<RpcConnectionSettings, RoomError>")]
pub struct RpcConnectionEstablished {
    /// ID of [`Member`] that establishes [`RpcConnection`].
    ///
    /// [`Member`]: crate::signalling::elements::Member
    pub member_id: MemberId,

    /// Credential of [`Member`] to authorize WebSocket connection with.
    ///
    /// [`Member`]: crate::signalling::elements::Member
    pub credentials: Credential,

    /// Established [`RpcConnection`].
    pub connection: Box<dyn RpcConnection>,
}
/// Signal of existing [`RpcConnection`] of specified [`Member`] being closed.
///
/// [`Member`]: crate::signalling::elements::Member
#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct RpcConnectionClosed {
    /// ID of [`Member`] which [`RpcConnection`] is closed.
    ///
    /// [`Member`]: crate::signalling::elements::Member
    pub member_id: MemberId,

    /// Reason of why [`RpcConnection`] is closed.
    pub reason: ClosedReason,
}

/// Signal of a [`Member`] which state needs synchronization.
///
/// [`Member`]: crate::signalling::elements::Member
#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct Synchronize(pub MemberId);

/// Reasons of why [`RpcConnection`] may be closed.
#[derive(Clone, Copy, Debug, PartialEq)]
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

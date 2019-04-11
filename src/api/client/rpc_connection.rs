//! ['RpcConnection'] with related messages.
use actix::Message;
use futures::Future;

use crate::{api::client::Event, api::control::Id as MemberId};

use std::fmt;

/// Abstraction over RPC connection with some remote [`Member`].
pub trait RpcConnection: fmt::Debug + Send {
    /// Closes [`RpcConnection`].
    /// No [`RpcConnectionClosed`] signals should be emitted.
    fn close(&mut self) -> Box<dyn Future<Item = (), Error = ()>>;

    /// Sends [`Event`] to remote [`Member`].
    fn send_event(
        &self,
        event: Event,
    ) -> Box<dyn Future<Item = (), Error = ()>>;
}

/// Signal for authorizing new [`RpcConnection`] before establishing.
#[derive(Debug, Message)]
#[rtype(result = "Result<(), RpcConnectionAuthorizationError>")]
pub struct AuthorizeRpcConnection {
    /// ID of [`Member`] to authorize [`RpcConnection`] for.
    pub member_id: MemberId,
    /// Credentials to authorize [`RpcConnection`] with.
    pub credentials: String, // TODO: &str when futures will allow references
}

/// Error of authorization [`RpcConnection`] in [`Room`].
#[derive(Debug)]
pub enum RpcConnectionAuthorizationError {
    /// Authorizing [`Member`] does not exists in the [`Room`].
    MemberNotExists,
    /// Provided credentials are invalid.
    InvalidCredentials,
}

/// Signal of new [`RpcConnection`] being established with specified [`Member`].
#[derive(Debug, Message)]
#[rtype(result = "Result<(), ()>")]
pub struct RpcConnectionEstablished {
    /// ID of [`Member`] that establishes [`RpcConnection`].
    pub member_id: MemberId,
    /// Established [`RpcConnection`].
    pub connection: Box<dyn RpcConnection>,
}
/// Signal of existing [`RpcConnection`] of specified [`Member`] being closed.
#[derive(Debug, Message)]
pub struct RpcConnectionClosed {
    /// ID of [`Member`] which [`RpcConnection`] is closed.
    pub member_id: MemberId,
    /// Reason of why [`RpcConnection`] is closed.
    pub reason: RpcConnectionClosedReason,
}

/// Reasons of why [`RpcConnection`] may be closed.
#[derive(Debug)]
pub enum RpcConnectionClosedReason {
    /// [`RpcConnection`] is disconnect by server itself.
    Disconnected,
    /// [`RpcConnection`] has become idle and is disconnected by idle timeout.
    Idle,
}

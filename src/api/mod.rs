//! API implementations provided by application.

pub mod client;
pub mod control;

use std::fmt::Debug;

use futures::future::LocalBoxFuture;
use medea_client_api_proto::{Command, MemberId, Token};

use crate::{
    api::client::rpc_connection::{
        ClosedReason, RpcConnection, RpcConnectionSettings,
    },
    signalling::room::RoomError,
};

/// Server side of Medea RPC protocol.
#[cfg_attr(test, mockall::automock)]
pub trait RpcServer: Debug + Send {
    /// Send signal of new [`RpcConnection`] being established with specified
    /// [`Member`]. Transport should consider dropping connection if message
    /// result is err.
    ///
    /// [`Member`]: crate::signalling::elements::member::Member
    fn connection_established(
        &self,
        member_id: MemberId,
        token: Token,
        connection: Box<dyn RpcConnection>,
    ) -> LocalBoxFuture<'static, Result<RpcConnectionSettings, ()>>;

    /// Send signal of existing [`RpcConnection`] of specified [`Member`] being
    /// closed.
    ///
    /// [`Member`]: crate::signalling::elements::member::Member
    fn connection_closed(
        &self,
        member_id: MemberId,
        reason: ClosedReason,
    ) -> LocalBoxFuture<'static, ()>;

    /// Sends [`Command`].
    fn send_command(&self, member_id: MemberId, msg: Command);
}

#[cfg(test)]
impl_debug_by_struct_name!(MockRpcServer);

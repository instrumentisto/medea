//! API implementations provided by application.

pub mod client;
pub mod control;

use std::fmt::Debug;

use futures::future::LocalBoxFuture;
use medea_client_api_proto::Command;

use crate::api::{
    client::rpc_connection::{ClosedReason, RpcConnection},
    control::MemberId,
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
        connection: Box<dyn RpcConnection>,
    ) -> LocalBoxFuture<'static, Result<(), ()>>;

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
    ///
    /// [`Command`]:
    fn send_command(
        &self,
        member_id: MemberId,
        msg: Command,
    ) -> LocalBoxFuture<'static, ()>;
}

#[cfg(test)]
impl_debug_by_struct_name!(MockRpcServer);

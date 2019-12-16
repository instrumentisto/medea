//! API implementations provided by application.

pub mod client;
pub mod control;

use std::fmt::Debug;

use futures::Future;
use medea_client_api_proto::Command;

use crate::api::client::rpc_connection::{
    RpcConnectionClosed, RpcConnectionEstablished,
};

#[cfg_attr(test, mockall::automock)]
pub trait RpcServer: Debug + Send {
    /// Send signal of new [`RpcConnection`] being established with specified
    /// [`Member`]. Transport should consider dropping connection if message
    /// result is err.
    ///
    /// [`Member`]: crate::signalling::elements::member::Member
    fn send_established(
        &self,
        msg: RpcConnectionEstablished,
    ) -> Box<dyn Future<Item = (), Error = ()>>;

    /// Send signal of existing [`RpcConnection`] of specified [`Member`] being
    /// closed. Cannot fail.
    ///
    /// [`Member`]: crate::signalling::elements::member::Member
    fn send_closed(
        &self,
        msg: RpcConnectionClosed,
    ) -> Box<dyn Future<Item = (), Error = ()>>;

    /// Sends [`Command`]. Cannot fail
    ///
    /// [`Command`]:
    fn send_command(
        &self,
        msg: Command,
    ) -> Box<dyn Future<Item = (), Error = ()>>;
}

#[cfg(test)]
impl Debug for MockRpcServer {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "MockRpcServer")
    }
}

//! API implementations provided by application.

pub mod client;
pub mod control;

use std::fmt::Debug;

use actix::Addr;
use futures::Future;

use crate::{
    api::client::rpc_connection::{
        CommandMessage, RpcConnectionClosed, RpcConnectionEstablished,
    },
    log::prelude::*,
    signalling::Room,
};

#[cfg_attr(test, mockall::automock)]
pub trait RpcServer: Debug + Send {
    fn send_established(
        &self,
        msg: RpcConnectionEstablished,
    ) -> Box<dyn Future<Item = (), Error = ()>>;

    fn send_closed(
        &self,
        msg: RpcConnectionClosed,
    ) -> Box<dyn Future<Item = (), Error = ()>>;

    fn send_command(
        &self,
        msg: CommandMessage,
    ) -> Box<dyn Future<Item = (), Error = ()>>;
}

#[cfg(test)]
impl Debug for MockRpcServer {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "MockRpcServer")
    }
}

impl RpcServer for Addr<Room> {
    fn send_established(
        &self,
        msg: RpcConnectionEstablished,
    ) -> Box<dyn Future<Item = (), Error = ()>> {
        Box::new(
            self.send(msg)
                .map_err(|err| {
                    error!(
                        "Failed to send RpcConnectionEstablished cause {:?}",
                        err,
                    );
                })
                .and_then(|result| result),
        )
    }

    fn send_closed(
        &self,
        msg: RpcConnectionClosed,
    ) -> Box<dyn Future<Item = (), Error = ()>> {
        Box::new(
            self.send(msg)
                .map_err(|err| {
                    error!(
                        "Failed to send RpcConnectionClosed cause {:?}",
                        err,
                    );
                })
                .then(|_| Ok(())),
        )
    }

    fn send_command(
        &self,
        msg: CommandMessage,
    ) -> Box<dyn Future<Item = (), Error = ()>> {
        Box::new(
            self.send(msg)
                .map_err(|err| {
                    error!(
                        "Failed to send CommandMessage cause {:?}",
                        err,
                    );
                })
                .then(|_| Ok(())),
        )
    }
}

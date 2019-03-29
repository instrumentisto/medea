use futures::Future;

use crate::api::{
    client::{Event, RpcConnection},
    control::member::Id as MemberId,
};

/// Session authorized [`Member`].
#[derive(Debug)]
pub struct Session {
    /// ID of [`Member`].
    pub member_id: MemberId,

    /// Established [`RpcConnection`]s of [`Member`]s.
    connection: Box<dyn RpcConnection>,
    /* TODO: Replace Box<dyn RpcConnection>> with enum,
     *       as the set of all possible RpcConnection types is not closed. */
}

impl Session {
    /// Returns new [`Session`] of [`Member`].
    pub fn new(
        member_id: MemberId,
        connection: Box<dyn RpcConnection>,
    ) -> Self {
        Session {
            member_id,
            connection,
        }
    }

    /// Sends [`Event`] to remote [`Member`].
    pub fn send_event(
        &self,
        event: Event,
    ) -> impl Future<Item = (), Error = ()> {
        self.connection.send_event(event)
    }

    /// Replace [`RpcConnection`] of [`Member`].
    ///
    /// Old [`RpcConnection`] will be close.
    pub fn set_connection(
        &mut self,
        connection: Box<dyn RpcConnection>,
    ) -> impl Future<Item = (), Error = ()> {
        let fut = self.connection.close();
        self.connection = connection;
        fut
    }
}

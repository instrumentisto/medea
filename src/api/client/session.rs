use futures::Future;

use crate::api::{
    client::{Event, RpcConnection},
    control::member::Id as MemberId,
};

#[derive(Debug)]
pub struct Session {
    pub member_id: MemberId,
    // TODO: Replace Box<dyn RpcConnection>> with enum,
    //       as the set of all possible RpcConnection types is not closed.
    connection: Box<dyn RpcConnection>,
}

impl Session {
    pub fn new(
        member_id: MemberId,
        connection: Box<dyn RpcConnection>,
    ) -> Self {
        Session {
            member_id,
            connection,
        }
    }

    pub fn send_event(
        &self,
        event: Event,
    ) -> impl Future<Item = (), Error = ()> {
        self.connection.send_event(event)
    }

    pub fn set_connection(
        &mut self,
        connection: Box<dyn RpcConnection>,
    ) -> impl Future<Item = (), Error = ()> {
        let fut = self.connection.close();
        self.connection = connection;
        fut
    }
}

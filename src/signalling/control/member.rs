use crate::api::client::rpc_connection::RpcConnection;
use crate::api::client::rpc_connection::EventMessage;

pub use crate::api::control::MemberId;
use failure::Fail;
use futures::Future;

#[derive(Debug)]
pub struct Member {
    id: MemberId,
    credentials: String,
    connection: Option<Box<dyn RpcConnection>>,
}

#[derive(Debug, Fail)]
pub enum MemberError {
    #[fail(display = "Rpc connection is empty.")]
    RpcConnectionEmpty
}

impl Member {
    pub fn close_connection(&mut self) -> Result<Box<dyn Future<Item = (), Error = ()>>, MemberError> {
        let connection = self.connection.as_mut().ok_or(MemberError::RpcConnectionEmpty)?;
        Ok(connection.close())
    }

    pub fn send_event(&mut self, event: EventMessage) -> Result<Box<dyn Future<Item = (), Error = ()>>, MemberError>{
        let connection = self.connection.as_mut().ok_or(MemberError::RpcConnectionEmpty)?;
        Ok(connection.send_event(event))
    }

    pub fn take_connection(&mut self) -> Option<Box<dyn RpcConnection>> {
        self.connection.take()
    }

    pub fn id(&self) -> MemberId {
        self.id.clone()
    }

    pub fn credentials(&self) -> String {
        self.credentials.clone()
    }

    pub fn set_connection(&mut self, connection: Box<dyn RpcConnection>) {
        self.connection = Some(connection)
    }

    pub fn remove_connection(&mut self) {
        self.connection = None;
    }

    pub fn is_connected(&self) -> bool {
        self.connection.is_some()
    }
}

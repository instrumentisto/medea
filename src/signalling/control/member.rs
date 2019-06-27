use crate::api::client::rpc_connection::RpcConnection;

pub use crate::api::control::MemberId;

#[derive(Debug)]
pub struct Member {
    id: MemberId,
    credentials: String,
    connection: Option<Box<dyn RpcConnection>>
}

impl Member {
    pub fn connection(&self) -> Option<&Box<dyn RpcConnection>> {
        self.connection.as_ref()
    }

    pub fn take_connection(&mut self) -> Option<Box<dyn RpcConnection>> {
        self.connection.take()
    }

    pub fn id(&self) -> &MemberId {
        &self.id
    }

    pub fn credentials(&self) -> &str {
        &self.credentials
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

use std::net::SocketAddr;

use crate::signalling::RoomId;
use medea_client_api_proto::IceServer;

/// Credentials on Turn server.
#[derive(Clone, Debug)]
pub struct IceUser {
    /// Address of Turn server.
    address: SocketAddr,
    //todo: this argument is passed by value, but not consumed in the function body. consider &str
    /// Username for authorization.
    name: String,
    /// Password for authorization.
    pass: String,
}

impl IceUser {
    pub fn new(
        address: SocketAddr,
        room_id: RoomId,
        name: String,
        pass: String,
    ) -> Self {
        Self {
            address,
            name: format!("{}:{}", name, room_id),
            pass,
        }
    }

    pub fn to_servers_list(&self) -> Vec<IceServer> {
        let stun_url = vec![format!("stun:{}", self.address)];
        let stun = IceServer {
            urls: stun_url,
            username: None,
            credential: None,
        };
        let turn_urls = vec![
            format!("turn:{}", self.address),
            format!("turn:{}?transport=tcp", self.address),
        ];
        let turn = IceServer {
            urls: turn_urls,
            username: Some(self.name.clone()),
            credential: Some(self.pass.clone()),
        };
        vec![stun, turn]
    }

    pub fn address(&self) -> &SocketAddr {
        &self.address
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn pass(&self) -> &String {
        &self.pass
    }
}

use std::net::SocketAddr;

use medea_client_api_proto::IceServer;

/// Credentials on Turn server.
#[derive(Clone, Debug)]
pub struct IceUser {
    /// Address of Turn server.
    pub address: SocketAddr,
    /// Username for authorization.
    pub name: String,
    /// Password for authorization.
    pub pass: String,
}

impl IceUser {
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
}

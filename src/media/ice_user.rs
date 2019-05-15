use std::net::SocketAddr;

use crate::api::protocol::ICEServer;

/// Credentials on Turn server.
#[derive(Clone, Debug)]
pub struct ICEUser {
    /// Address of Turn server.
    pub address: SocketAddr,
    /// Username for authorization.
    pub name: String,
    /// Password for authorization.
    pub pass: String,
}

impl Into<Vec<ICEServer>> for ICEUser {
    fn into(self) -> Vec<ICEServer> {
        let stun_url = vec![format!("stun:{}", self.address)];
        let stun = ICEServer {
            urls: stun_url,
            username: None,
            credential: None,
        };
        let turn_urls = vec![
            format!("turn:{}", self.address),
            format!("turn:{}?transport=tcp", self.address),
        ];
        let turn = ICEServer {
            urls: turn_urls,
            username: Some(self.name.clone()),
            credential: Some(self.pass.clone()),
        };
        vec![stun, turn]
    }
}

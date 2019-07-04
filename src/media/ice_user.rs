use std::net::SocketAddr;

use medea_client_api_proto::IceServer;

use crate::api::control::serde::RoomId;

/// Credentials on Turn server.
#[derive(Clone, Debug)]
pub struct IceUser {
    /// Address of Turn server.
    address: SocketAddr,
    /// Username for authorization.
    user: String,
    /// Password for authorization.
    pass: String,
    /// Non static users are meant to be saved and delete from some remote
    /// storage, while static users are hardcoded on Turn server and do not
    /// require any additional management.
    is_static: bool,
}

impl IceUser {
    /// Build new non static [`IceUser`].
    pub fn build(
        address: SocketAddr,
        room_id: &RoomId,
        name: &str,
        pass: String,
    ) -> Self {
        Self {
            address,
            user: format!("{}_{}", room_id, name),
            pass,
            is_static: false,
        }
    }

    /// Build new static [`IceUser`].
    pub fn new(address: SocketAddr, user: String, pass: String) -> Self {
        Self {
            address,
            user,
            pass,
            is_static: true,
        }
    }

    /// Build vector of [`IceServer`].
    pub fn servers_list(&self) -> Vec<IceServer> {
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
            username: Some(self.user.clone()),
            credential: Some(self.pass.clone()),
        };
        vec![stun, turn]
    }

    pub fn address(&self) -> &SocketAddr {
        &self.address
    }

    pub fn user(&self) -> &str {
        &self.user
    }

    pub fn pass(&self) -> &str {
        &self.pass
    }

    pub fn is_static(&self) -> bool {
        self.is_static
    }
}

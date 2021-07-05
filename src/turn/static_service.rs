use async_trait::async_trait;
use crate::turn::{TurnAuthService, TurnServiceErr, IceUser, UnreachablePolicy};
use medea_client_api_proto::{RoomId, PeerId};

#[derive(Debug)]
enum Kind {
    Turn,
    Stun,
}

#[derive(Debug)]
pub struct StaticIceUser {
    address: String,
    username: Option<String>,
    pass: Option<String>,
    kind: Kind,
}

#[derive(Debug)]
pub struct StaticService {
    ice_users: Vec<StaticIceUser>,
}

#[async_trait]
impl TurnAuthService for StaticService {
    async fn create(&self, room_id: RoomId, peer_id: PeerId, policy: UnreachablePolicy) -> Result<IceUser, TurnServiceErr> {
        todo!()
    }
}

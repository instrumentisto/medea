use crate::{
    conf,
    conf::turn::StaticCredentials,
    turn::{IceUser, TurnAuthService, TurnServiceErr, UnreachablePolicy},
};
use async_trait::async_trait;
use derive_more::Display;
use medea_client_api_proto::{IceServer, PeerId, RoomId};

#[derive(Debug, Display, Clone, Copy)]
enum Kind {
    #[display(fmt = "turn")]
    Turn,
    #[display(fmt = "stun")]
    Stun,
}

#[derive(Debug, Clone)]
pub struct StaticIceUser {
    address: String,
    username: Option<String>,
    pass: Option<String>,
    kind: Kind,
}

impl StaticIceUser {
    fn new(creds: StaticCredentials, kind: Kind) -> Self {
        Self {
            address: creds.address,
            username: creds.username,
            pass: creds.pass,
            kind,
        }
    }
}

impl StaticIceUser {
    pub fn ice_server(&self) -> IceServer {
        let stun_url = vec![format!("{}:{}", self.kind, self.address)];
        IceServer {
            urls: stun_url,
            username: self.username.clone(),
            credential: self.pass.clone(),
        }
    }
}

#[derive(Debug)]
pub struct StaticService {
    ice_users: Vec<StaticIceUser>,
}

impl StaticService {
    pub fn new(cf: &conf::turn::Static) -> Self {
        Self {
            ice_users: cf
                .stuns
                .iter()
                .map(|creds| StaticIceUser::new(creds.clone(), Kind::Stun))
                .chain(
                    cf.turns.iter().map(|creds| {
                        StaticIceUser::new(creds.clone(), Kind::Turn)
                    }),
                )
                .collect(),
        }
    }
}

#[async_trait]
impl TurnAuthService for StaticService {
    async fn create(
        &self,
        _: RoomId,
        _: PeerId,
        _: UnreachablePolicy,
    ) -> Result<Vec<IceUser>, TurnServiceErr> {
        Ok(self.ice_users.iter().cloned().map(|i| i.into()).collect())
    }
}

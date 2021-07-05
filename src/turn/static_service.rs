//! Implementation of the [`TurnAuthService`] with static [ICE] users.
//!
//! [ICE]: https://webrtcglossary.com/ice/

use async_trait::async_trait;
use derive_more::Display;
use medea_client_api_proto::{IceServer, PeerId, RoomId};

use crate::{
    conf,
    conf::turn::StaticCredentials,
    turn::{IceUser, TurnAuthService, TurnServiceErr, UnreachablePolicy},
};

/// Kind of [`StaticIceUser`].
#[derive(Debug, Display, Clone, Copy)]
enum Kind {
    /// This is [TURN] [ICE] user.
    ///
    /// [TURN]: https://webrtcglossary.com/turn/
    /// [ICE]: https://webrtcglossary.com/ice/
    #[display(fmt = "turn")]
    Turn,

    /// This is [STUN] [ICE] user.
    ///
    /// [STUN]: https://webrtcglossary.com/stun/
    /// [ICE]: https://webrtcglossary.com/ice/
    #[display(fmt = "stun")]
    Stun,
}

/// Static [ICE] user credentials which will be provided to the client.
#[derive(Debug, Clone)]
pub struct StaticIceUser {
    /// Address of Turn server.
    address: String,

    /// Username for authorization.
    username: Option<String>,

    /// Password for authorization.
    pass: Option<String>,

    /// Kind of this [`StaticIceUser`].
    kind: Kind,
}

impl StaticIceUser {
    /// Returns new [`StaticIceUser`] with a provided credentials and [`Kind`].
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
    /// Returns [`IceServer`] of this [`StaticIceUser`].
    pub fn ice_server(&self) -> IceServer {
        IceServer {
            urls: vec![format!("{}:{}", self.kind, self.address)],
            username: self.username.clone(),
            credential: self.pass.clone(),
        }
    }
}

/// Service which implements [`TurnAuthService`] with static [ICE] users.
///
/// [ICE]: https://webrtcglossary.com/ice/
#[derive(Debug)]
pub struct StaticService(Vec<StaticIceUser>);

impl StaticService {
    /// Returns new [`StaticService`] based on the provided
    /// [`conf::turn::Static`].
    pub fn new(cf: &conf::turn::Static) -> Self {
        Self(
            cf.stuns
                .iter()
                .map(|creds| StaticIceUser::new(creds.clone(), Kind::Stun))
                .chain(
                    cf.turns.iter().map(|creds| {
                        StaticIceUser::new(creds.clone(), Kind::Turn)
                    }),
                )
                .collect(),
        )
    }
}

#[async_trait]
impl TurnAuthService for StaticService {
    /// Returns all [`IceUser`]s from this [`StaticService`].
    async fn create(
        &self,
        _: RoomId,
        _: PeerId,
        _: UnreachablePolicy,
    ) -> Result<Vec<IceUser>, TurnServiceErr> {
        Ok(self.0.iter().cloned().map(|i| i.into()).collect())
    }
}

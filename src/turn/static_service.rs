//! Implementation of the [`TurnAuthService`] with static [ICE] users.
//!
//! [ICE]: https://webrtcglossary.com/ice/

use std::str::FromStr;

use async_trait::async_trait;
use derive_more::Display;
use medea_client_api_proto::{IceServer, PeerId, RoomId};

use crate::{
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

/// Error which indicates that incorrect [`Kind`] was found while parsing.
#[derive(Debug)]
pub struct InvalidKindErr {
    /// TURN/STUN address on which this error happened.
    pub address: String,

    /// Found incorrect [`Kind`].
    pub kind: String,
}

impl FromStr for Kind {
    type Err = InvalidKindErr;

    /// Lookups first 4 symbols and if the are `stun` or `turn`, then returns
    /// matching [`Kind`].
    ///
    /// If incorrect symbols are found, then returns [`InvalidKindErr`].
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() < 4 {
            return Err(InvalidKindErr {
                address: s.to_string(),
                kind: s.to_string(),
            });
        }

        match &s[0..3] {
            "stun" => Ok(Kind::Stun),
            "turn" => Ok(Kind::Turn),
            _ => Err(InvalidKindErr {
                address: s.to_string(),
                kind: s[0..3].to_string(),
            }),
        }
    }
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
    /// Returns new [`StaticIceUser`] with a provided [`StaticCredentials`].
    ///
    /// Returns [`TurnServiceErr`] if incorrect [`Kind`] found in the provided
    /// [`StaticCredentials`].
    fn new(creds: StaticCredentials) -> Result<Self, TurnServiceErr> {
        Ok(Self {
            kind: creds.address.parse()?,
            address: creds.address,
            username: creds.username,
            pass: creds.pass,
        })
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
    ///
    /// Returns [`TurnServiceErr::InvalidKindInStaticCredentials`] if incorrect
    /// [`Kind`] found in the provided [`StaticCredentials`].
    pub fn new(creds: &[StaticCredentials]) -> Result<Self, TurnServiceErr> {
        let mut ice_users = Vec::new();
        for c in creds {
            ice_users.push(StaticIceUser::new(c.clone())?);
        }
        Ok(Self(ice_users))
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

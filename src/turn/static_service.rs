//! Implementation of the [`TurnAuthService`] with static [ICE] users.
//!
//! [ICE]: https://webrtcglossary.com/ice/

use async_trait::async_trait;
use medea_client_api_proto::{IceServer, PeerId, RoomId};

use crate::{
    conf::turn::RtcIceServer,
    turn::{IceUser, TurnAuthService, TurnServiceErr, UnreachablePolicy},
};

/// Static [ICE] user credentials which will be provided to the client.
#[derive(Debug, Clone)]
pub struct StaticIceUser {
    /// URLs which can be used to connect to the server.
    urls: Vec<String>,

    /// Username to use during the authentication process.
    username: Option<String>,

    /// The credential to use when logging into the server.
    credential: Option<String>,
}

impl StaticIceUser {
    /// Returns new [`StaticIceUser`] with the provided [`RtcIceServers`].
    ///
    /// Returns [`TurnServiceErr`] if incorrect [`Kind`] found in the provided
    /// [`RtcIceServers`].
    fn new(servers: RtcIceServer) -> Self {
        Self {
            urls: servers.urls,
            username: servers.username,
            credential: servers.credential,
        }
    }
}

impl StaticIceUser {
    /// Returns [`IceServer`] of this [`StaticIceUser`].
    pub fn ice_server(&self) -> IceServer {
        IceServer {
            urls: self.urls.clone(),
            username: self.username.clone(),
            credential: self.credential.clone(),
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
    pub fn new(servers: Vec<RtcIceServer>) -> Self {
        Self(servers.into_iter().map(StaticIceUser::new).collect())
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

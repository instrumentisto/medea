//! Implementation of the [`TurnAuthService`] with static [ICE] users.
//!
//! [ICE]: https://webrtcglossary.com/ice

use async_trait::async_trait;
use medea_client_api_proto::{IceServer, PeerId, RoomId};

use crate::{
    conf,
    turn::{IceUser, TurnAuthService, TurnServiceErr, UnreachablePolicy},
};

/// Static [ICE] user credentials which will be provided to the client.
///
/// [ICE]: https://webrtcglossary.com/ice
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
    /// Creates a new [`StaticIceUser`] out of the the provided
    /// [`conf::ice::Server`] options.
    #[inline]
    #[must_use]
    fn new(servers: conf::ice::Server) -> Self {
        Self {
            urls: servers.urls.into_iter().map(Into::into).collect(),
            username: servers.user.map(Into::into),
            credential: servers.pass.map(Into::into),
        }
    }
}

impl StaticIceUser {
    /// Returns [`IceServer`] of this [`StaticIceUser`].
    #[inline]
    #[must_use]
    pub fn ice_server(&self) -> IceServer {
        IceServer {
            urls: self.urls.clone(),
            username: self.username.clone(),
            credential: self.credential.clone(),
        }
    }
}

/// Service implementing [`TurnAuthService`] with static [ICE] users.
///
/// [ICE]: https://webrtcglossary.com/ice
#[derive(Debug)]
pub struct StaticService(Vec<StaticIceUser>);

impl StaticService {
    /// Returns a new [`StaticService`] based on the provided
    /// [`conf::ice::Server`] options.
    #[inline]
    #[must_use]
    pub fn new(servers: Vec<conf::ice::Server>) -> Self {
        Self(servers.into_iter().map(StaticIceUser::new).collect())
    }
}

#[async_trait]
impl TurnAuthService for StaticService {
    /// Returns all the [`IceUser`]s from this [`StaticService`].
    ///
    /// # Errors
    ///
    /// Never errors.
    async fn create(
        &self,
        _: RoomId,
        _: PeerId,
        _: UnreachablePolicy,
    ) -> Result<Vec<IceUser>, TurnServiceErr> {
        Ok(self.0.iter().cloned().map(|i| i.into()).collect())
    }
}

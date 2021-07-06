//! Implementation of entity which represents credentials on [TURN]/[STUN]
//! server.
//!
//! [TURN]: https://webrtcglossary.com/turn/
//! [STUN]: https://webrtcglossary.com/stun/

use std::convert::TryFrom;

use derive_more::From;
use medea_client_api_proto::IceServer;

use crate::turn::static_service::StaticIceUser;

use super::coturn::CoturnIceUser;

/// Error which indicates that [`IceUsers`] is empty.
#[derive(Debug)]
pub struct EmptyIceServersListErr;

/// List of [`IceUser`] created for some [`Peer`].
///
/// [`Peer`]: crate::media::peer::Peer
#[derive(Debug)]
pub struct IceUsers(Vec<IceUser>);

impl IceUsers {
    /// Returns new empty [`IceUsers`] list.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Adds provided [`IceUser`]s to this [`IceUsers`] list.
    #[inline]
    pub fn add(&mut self, mut users: Vec<IceUser>) {
        self.0.append(&mut users);
    }
}

impl TryFrom<&IceUsers> for Vec<IceServer> {
    type Error = EmptyIceServersListErr;

    /// Returns vector of [`IceServer`]s if [`IceUsers`] list is not empty.
    ///
    /// Otherwise, returns [`EmptyIceServersListErr`].
    fn try_from(value: &IceUsers) -> Result<Self, Self::Error> {
        if value.0.is_empty() {
            Err(EmptyIceServersListErr)
        } else {
            Ok(value.0.iter().flat_map(IceUser::servers_list).collect())
        }
    }
}

impl Default for IceUsers {
    fn default() -> Self {
        Self::new()
    }
}

/// Credentials on Turn server.
#[derive(Debug, From)]
pub enum IceUser {
    /// [ICE] user on [Coturn] [TURN]/[STUN] server.
    ///
    /// [ICE]: https://webrtcglossary.com/ice/
    /// [Coturn]: https://github.com/coturn/coturn
    /// [TURN]: https://webrtcglossary.com/turn/
    /// [STUN]: https://webrtcglossary.com/stun/
    Coturn(CoturnIceUser),

    /// Static [ICE] user on some [TURN]/[STUN] server.
    ///
    /// [ICE]: https://webrtcglossary.com/ice/
    /// [TURN]: https://webrtcglossary.com/turn/
    /// [STUN]: https://webrtcglossary.com/stun/
    Static(StaticIceUser),
}

impl IceUser {
    /// Returns [`IceServer`]s of this [`IceUser`].
    #[must_use]
    pub fn servers_list(&self) -> Vec<IceServer> {
        match self {
            Self::Coturn(coturn) => coturn.servers_list(),
            Self::Static(user) => {
                vec![user.ice_server()]
            }
        }
    }
}

#[cfg(test)]
impl IceUser {
    /// Returns new [Coturn] static [`IceUser`] with a provided credentials.
    ///
    /// [Coturn]: https://github.com/coturn/coturn
    pub fn new_coturn_static(
        address: String,
        username: String,
        pass: String,
    ) -> Self {
        Self::Coturn(CoturnIceUser::new_static(address, username, pass))
    }
}

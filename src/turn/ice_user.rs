//! Implementation of entity which represents credentials on [TURN]/[STUN]
//! server.
//!
//! [TURN]: https://webrtcglossary.com/turn/
//! [STUN]: https://webrtcglossary.com/stun/

use derive_more::From;
use medea_client_api_proto::IceServer;

use crate::turn::static_service::StaticIceUser;

use super::coturn::CoturnIceUser;

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

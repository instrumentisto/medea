use super::coturn::CoturnIceUser;
use crate::turn::static_service::StaticIceUser;
use medea_client_api_proto::IceServer;

#[derive(Debug)]
pub enum IceUser {
    Coturn(CoturnIceUser),
    Static(StaticIceUser),
}

impl IceUser {
    pub fn servers_list(&self) -> Vec<IceServer> {
        match self {
            Self::Coturn(coturn) => coturn.servers_list(),
            Self::Static(user) => {
                vec![user.ice_server()]
            }
        }
    }
}

impl From<CoturnIceUser> for IceUser {
    fn from(from: CoturnIceUser) -> Self {
        Self::Coturn(from)
    }
}

impl From<StaticIceUser> for IceUser {
    fn from(from: StaticIceUser) -> Self {
        Self::Static(from)
    }
}

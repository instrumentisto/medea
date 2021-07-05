use super::coturn::CoturnIceUser as CoturnIceUser;
use medea_client_api_proto::IceServer;

#[derive(Debug)]
pub enum IceUser {
    Coturn(CoturnIceUser),
}

impl IceUser {
    pub fn servers_list(&self) -> Vec<IceServer> {
        match self {
            Self::Coturn(coturn) => coturn.servers_list(),
        }
    }
}

impl From<CoturnIceUser> for IceUser {
    fn from(from: CoturnIceUser) -> Self {
        Self::Coturn(from)
    }
}

use dart_sys::Dart_Handle;
use medea_client_api_proto::{IceConnectionState, PeerConnectionState};
use tracerr::Traced;

use crate::{
    media::MediaKind,
    platform::{
        dart::transceiver::Transceiver, peer_connection::RtcSdpType,
        RtcPeerConnectionError, TransceiverDirection,
    },
};

type Result<T> = std::result::Result<T, Traced<RtcPeerConnectionError>>;

#[derive(Clone, Debug)]
pub struct RtcPeerConnection {
    handle: Dart_Handle,
}

impl RtcPeerConnection {
    pub fn ice_connection_state(&self) -> IceConnectionState {
        todo!()
    }

    pub fn connection_state(&self) -> Option<PeerConnectionState> {
        todo!()
    }

    pub async fn add_ice_candidate(
        &self,
        candidate: &str,
        sdp_m_line_index: Option<u16>,
        sdp_mid: &Option<String>,
    ) -> Result<()> {
        todo!()
    }

    pub fn restart_ice(&self) {
        todo!()
    }

    pub async fn set_offer(&self, offer: &str) -> Result<()> {
        todo!()
    }

    pub async fn set_answer(&self, answer: &str) -> Result<()> {
        todo!()
    }

    pub async fn create_answer(&self) -> Result<()> {
        todo!()
    }

    pub async fn rollback(&self) -> Result<()> {
        todo!()
    }

    pub async fn create_offer(&self) -> Result<String> {
        todo!()
    }

    pub async fn set_remote_description(&self, sdp: RtcSdpType) -> Result<()> {
        todo!()
    }

    pub fn add_transceiver(
        &self,
        kind: MediaKind,
        direction: TransceiverDirection,
    ) -> Transceiver {
        todo!()
    }

    pub fn get_transceiver_by_mid(&self, mid: &str) -> Option<Transceiver> {
        todo!()
    }
}

use dart_sys::Dart_Handle;
use medea_client_api_proto::{IceConnectionState, PeerConnectionState};
use tracerr::Traced;

use crate::{
    media::MediaKind,
    platform::{
        dart::transceiver::Transceiver, peer_connection::RtcSdpType,
        RtcPeerConnectionError, SdpType, TransceiverDirection,
    },
    utils::dart::into_dart_string,
};

type Result<T> = std::result::Result<T, Traced<RtcPeerConnectionError>>;

type IceConnectionStateFunction = extern "C" fn(Dart_Handle) -> i32;
static mut ICE_CONNECTION_STATE_FUNCTION: Option<IceConnectionStateFunction> =
    None;

#[no_mangle]
pub unsafe extern "C" fn register_RtcPeerConnection__ice_connection_state(
    f: IceConnectionStateFunction,
) {
    ICE_CONNECTION_STATE_FUNCTION = Some(f);
}

type ConnectionStateFunction = extern "C" fn(Dart_Handle) -> i32;
static mut CONNECTION_STATE_FUNCTION: Option<ConnectionStateFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_RtcPeerConnection__connection_state(
    f: ConnectionStateFunction,
) {
    CONNECTION_STATE_FUNCTION = Some(f);
}

type RestartIceFunction = extern "C" fn(Dart_Handle);
static mut RESTART_ICE_FUNCTION: Option<RestartIceFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_RtcPeerConnection__restart_ice(
    f: RestartIceFunction,
) {
    RESTART_ICE_FUNCTION = Some(f);
}

type SetOfferFunction = extern "C" fn(Dart_Handle, *const libc::c_char);
static mut SET_OFFER_FUNCTION: Option<SetOfferFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_RtcPeerConnection__set_offer(
    f: SetOfferFunction,
) {
    SET_OFFER_FUNCTION = Some(f);
}

type RollbackFunction = extern "C" fn(Dart_Handle);
static mut ROLLBACK_FUNCTION: Option<RollbackFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_RtcPeerConnection__rollback(
    f: RollbackFunction,
) {
    ROLLBACK_FUNCTION = Some(f);
}

type GetTransceiverFunction =
    extern "C" fn(Dart_Handle, i32, i32) -> Dart_Handle;
static mut GET_TRANSCEIVER_FUNCTION: Option<GetTransceiverFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_RtcPeerConnection__get_transceiver(
    f: GetTransceiverFunction,
) {
    GET_TRANSCEIVER_FUNCTION = Some(f);
}

type GetTransceiverByMid =
    extern "C" fn(Dart_Handle, *const libc::c_char) -> Dart_Handle;
static mut GET_TRANSCEIVER_BY_MID_FUNCTION: Option<GetTransceiverByMid> = None;

#[no_mangle]
pub unsafe extern "C" fn register_RtcPeerConnection__get_transceiver_by_mid(
    f: GetTransceiverByMid,
) {
    GET_TRANSCEIVER_BY_MID_FUNCTION = Some(f);
}

type SetLocalDescriptionFunction =
    extern "C" fn(Dart_Handle, i32, *const libc::c_char);
static mut SET_LOCAL_DESCRIPTION_FUNCTION: Option<SetLocalDescriptionFunction> =
    None;

#[no_mangle]
pub unsafe extern "C" fn register_RtcPeerConnection__set_local_description(
    f: SetLocalDescriptionFunction,
) {
    SET_LOCAL_DESCRIPTION_FUNCTION = Some(f);
}

type SetRemoteDescriptionFunction =
    extern "C" fn(Dart_Handle, i32, *const libc::c_char);
static mut SET_REMOTE_DESCRIPTION_FUNCTION: Option<
    SetRemoteDescriptionFunction,
> = None;

#[no_mangle]
pub unsafe extern "C" fn register_RtcPeerConnection__set_remote_description(
    f: SetRemoteDescriptionFunction,
) {
    SET_REMOTE_DESCRIPTION_FUNCTION = Some(f);
}

#[derive(Clone, Debug)]
pub struct RtcPeerConnection {
    handle: Dart_Handle,
}

impl RtcPeerConnection {
    pub fn ice_connection_state(&self) -> IceConnectionState {
        unsafe {
            let ice_connection_state =
                ICE_CONNECTION_STATE_FUNCTION.unwrap()(self.handle);
            match ice_connection_state {
                0 => IceConnectionState::New,
                1 => IceConnectionState::Checking,
                2 => IceConnectionState::Connected,
                3 => IceConnectionState::Completed,
                4 => IceConnectionState::Failed,
                5 => IceConnectionState::Disconnected,
                6 => IceConnectionState::Closed,
                _ => unreachable!(),
            }
        }
    }

    pub fn connection_state(&self) -> Option<PeerConnectionState> {
        unsafe {
            let connection_state =
                CONNECTION_STATE_FUNCTION.unwrap()(self.handle);
            Some(match connection_state {
                0 => PeerConnectionState::New,
                1 => PeerConnectionState::Connecting,
                2 => PeerConnectionState::Connected,
                3 => PeerConnectionState::Disconnected,
                4 => PeerConnectionState::Failed,
                5 => PeerConnectionState::Closed,
                _ => unreachable!(),
            })
        }
    }

    pub async fn add_ice_candidate(
        &self,
        candidate: &str,
        sdp_m_line_index: Option<u16>,
        sdp_mid: &Option<String>,
    ) -> Result<()> {
        todo!("Need to do something with nullable arguments")
    }

    pub fn restart_ice(&self) {
        unsafe { RESTART_ICE_FUNCTION.unwrap()(self.handle) };
    }

    pub async fn set_offer(&self, offer: &str) -> Result<()> {
        self.set_local_description(RtcSdpType::Offer, offer.to_string());
        // TODO: result
        Ok(())
    }

    pub async fn set_answer(&self, answer: &str) -> Result<()> {
        self.set_local_description(RtcSdpType::Answer, answer.to_string());
        // TODO: result
        Ok(())
    }

    fn set_local_description(&self, sdp_type: RtcSdpType, sdp: String) {
        unsafe {
            SET_LOCAL_DESCRIPTION_FUNCTION.unwrap()(
                self.handle,
                sdp_type.into(),
                into_dart_string(sdp),
            );
        }
    }

    pub async fn set_remote_description(&self, sdp: SdpType) -> Result<()> {
        match sdp {
            SdpType::Offer(sdp) => unsafe {
                SET_REMOTE_DESCRIPTION_FUNCTION.unwrap()(
                    self.handle,
                    RtcSdpType::Offer.into(),
                    into_dart_string(sdp),
                );
            },
            SdpType::Answer(sdp) => unsafe {
                SET_REMOTE_DESCRIPTION_FUNCTION.unwrap()(
                    self.handle,
                    RtcSdpType::Answer.into(),
                    into_dart_string(sdp),
                );
            },
        }
        Ok(())
    }

    pub async fn create_answer(&self) -> Result<()> {
        todo!("Should be backed by the same function as create_offer")
    }

    pub async fn create_offer(&self) -> Result<String> {
        todo!("Should be backed by the same function as create_answer")
    }

    pub async fn rollback(&self) -> Result<()> {
        unsafe { ROLLBACK_FUNCTION.unwrap()(self.handle) };
        // TODO: Result?????
        Ok(())
    }

    pub fn add_transceiver(
        &self,
        kind: MediaKind,
        direction: TransceiverDirection,
    ) -> Transceiver {
        unsafe {
            let dir = if direction.is_all() {
                0
            } else if direction.contains(TransceiverDirection::RECV) {
                1
            } else if direction.contains(TransceiverDirection::SEND) {
                2
            } else {
                3
            };
            Transceiver::from(GET_TRANSCEIVER_FUNCTION.unwrap()(
                self.handle,
                kind.into(),
                dir,
            ))
        }
    }

    pub fn get_transceiver_by_mid(&self, mid: &str) -> Option<Transceiver> {
        unsafe {
            let transceiver = GET_TRANSCEIVER_BY_MID_FUNCTION.unwrap()(
                self.handle,
                into_dart_string(mid.to_string()),
            );
            if transceiver.is_null() {
                None
            } else {
                Some(Transceiver::from(transceiver))
            }
        }
    }
}

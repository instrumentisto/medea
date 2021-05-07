use medea_client_api_proto::{IceConnectionState, PeerConnectionState};

pub mod callback;
pub mod callback_listener;
pub mod handle;
pub mod list;
pub mod map;
pub mod nullable;
pub mod option;
pub mod result;
pub mod completer;
pub mod dart_api;

pub use self::callback::{Callback, Function};

pub fn ice_connection_from_int(i: i32) -> IceConnectionState {
    match i {
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

pub fn peer_connection_state_from_int(i: i32) -> PeerConnectionState {
    match i {
        0 => PeerConnectionState::New,
        1 => PeerConnectionState::Connecting,
        2 => PeerConnectionState::Connected,
        3 => PeerConnectionState::Disconnected,
        4 => PeerConnectionState::Failed,
        5 => PeerConnectionState::Closed,
        _ => unreachable!(),
    }
}

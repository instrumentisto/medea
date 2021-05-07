//! Platform-specific functionality.

pub mod callback;
pub mod peer_connection;
pub mod rtc_stats;
pub mod transceiver;
pub mod transport;

cfg_if::cfg_if! {
    if #[cfg(target_os = "android")] {
        mod dart;
        pub use self::dart::*;
    } else {
        mod wasm;
        pub use self::wasm::*;
    }
}

pub use self::{
    callback::Callback,
    peer_connection::{IceCandidate, RtcPeerConnectionError, SdpType},
    rtc_stats::RtcStatsError,
    transceiver::TransceiverDirection,
    transport::{RpcTransport, TransportError, TransportState},
};

#[cfg(feature = "mockable")]
pub use self::transport::MockRpcTransport;

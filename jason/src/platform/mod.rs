//! Platform-specific functionality.

pub mod peer_connection;
pub mod rtc_stats;
pub mod transport;

cfg_if::cfg_if! {
    if #[cfg(target_os = "android")] {
        mod dart_ffi;
        pub use self::dart_ffi::*;
    } else {
        mod wasm;
        pub use self::wasm::*;
    }
}

pub use self::{
    peer_connection::{IceCandidate, RtcPeerConnectionError, SdpType},
    rtc_stats::RtcStatsError,
    transport::{RpcTransport, TransportError, TransportState},
};

#[cfg(feature = "mockable")]
pub use self::transport::MockRpcTransport;

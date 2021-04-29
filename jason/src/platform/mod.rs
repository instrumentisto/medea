//! Platform-specific functionality.

pub mod peer_connection;
pub mod rtc_stats;
pub mod transceiver_direction;
pub mod transport;

cfg_if::cfg_if! {
    if #[cfg(target_os = "android")] {
        // TODO: make it private
        pub mod dart;
        pub use self::dart::{
            constraints::{DisplayMediaStreamConstraints, MediaStreamConstraints},
            delay_for,
            input_device_info::InputDeviceInfo,
            media_devices::{enumerate_devices, get_display_media, get_user_media},
            media_track::MediaStreamTrack,
            peer_connection::RtcPeerConnection,
            spawn,
            transceiver::Transceiver,
            utils::{Callback, Function},
            error::Error,
            rtc_stats::RtcStats,
            set_panic_hook,
            transport::WebSocketRpcTransport,
        };
    } else {
        mod wasm;
        pub use self::wasm::{
            constraints::{DisplayMediaStreamConstraints, MediaStreamConstraints},
            delay_for,
            error::Error,
            init_logger,
            input_device_info::InputDeviceInfo,
            media_devices::{enumerate_devices, get_display_media, get_user_media},
            media_track::MediaStreamTrack,
            peer_connection::RtcPeerConnection,
            rtc_stats::RtcStats,
            set_panic_hook, spawn,
            transceiver::{Transceiver},
            utils::{Callback, Function},
            transport::WebSocketRpcTransport,
        };
    }
}

pub use self::{
    peer_connection::{IceCandidate, RtcPeerConnectionError, SdpType},
    rtc_stats::RtcStatsError,
    transceiver_direction::TransceiverDirection,
    transport::{RpcTransport, TransportError, TransportState},
};

#[cfg(feature = "mockable")]
pub use self::transport::MockRpcTransport;

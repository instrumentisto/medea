//! Platform-specific functionality.

pub mod peer_connection;
pub mod rtc_stats;
pub mod transport;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub mod wasm;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub use self::{
    peer_connection::{IceCandidate, RtcPeerConnectionError, SdpType},
    rtc_stats::RtcStatsError,
    transport::{
        RpcTransport, TransportError, TransportState, WebSocketRpcTransport,
    },
    wasm::{
        constraints::{DisplayMediaStreamConstraints, MediaStreamConstraints},
        delay_for,
        error::Error,
        get_property_by_name, init_logger,
        input_device_info::InputDeviceInfo,
        media_devices::{enumerate_devices, get_display_media, get_user_media},
        media_track::MediaStreamTrack,
        peer_connection::RtcPeerConnection,
        rtc_stats::RtcStats,
        set_panic_hook, spawn,
        transceiver::{Transceiver, TransceiverDirection},
        utils::{Callback, Function},
    },
};

#[cfg(all(target_os = "android"))]
pub mod dart_ffi;

#[cfg(all(target_os = "android"))]
pub use self::{
    dart_ffi::{
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
        transceiver::{Transceiver, TransceiverDirection},
        utils::{Callback, Function},
    },
    peer_connection::{IceCandidate, RtcPeerConnectionError, SdpType},
    rtc_stats::RtcStatsError,
    transport::{
        RpcTransport, TransportError, TransportState, WebSocketRpcTransport,
    },
};

#[cfg(feature = "mockable")]
pub use self::transport::MockRpcTransport;

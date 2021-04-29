//! Platform-specific functionality.

#[cfg(all(target_os = "android"))]
pub mod dart;
pub mod peer_connection;
pub mod rtc_stats;
pub mod transceiver_direction;
pub mod transport;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub mod wasm;

pub use self::{
    peer_connection::{IceCandidate, RtcPeerConnectionError, SdpType},
    rtc_stats::RtcStatsError,
    transceiver_direction::TransceiverDirection,
    transport::{
        RpcTransport, TransportError, TransportState, WebSocketRpcTransport,
    },
};

// #[cfg(feature = "dart")]
// pub use self::dart::{
//     constraints::{DisplayMediaStreamConstraints, MediaStreamConstraints},
//     delay_for,
//     error::Error,
//     input_device_info::InputDeviceInfo,
//     media_devices::{enumerate_devices, get_display_media, get_user_media},
//     media_track::MediaStreamTrack,
//     peer_connection::RtcPeerConnection,
//     spawn,
//     transceiver::Transceiver,
//     utils::{callback::Callback, callback::Function},
// };
#[cfg(all(target_os = "android"))]
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
};
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub use self::wasm::{error::Error, rtc_stats::RtcStats};

#[cfg(feature = "mockable")]
pub use self::transport::MockRpcTransport;

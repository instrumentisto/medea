//! Platform-specific functionality.

#[cfg(feature = "dart")]
pub mod dart;
pub mod peer_connection;
pub mod rtc_stats;
pub mod transceiver_direction;
pub mod transport;
#[cfg(feature = "wasm")]
pub mod wasm;

pub use self::{
    peer_connection::{IceCandidate, RtcPeerConnectionError, SdpType},
    rtc_stats::RtcStatsError,
    transceiver_direction::TransceiverDirection,
    transport::{
        RpcTransport, TransportError, TransportState, WebSocketRpcTransport,
    },
};

#[cfg(feature = "dart")]
pub use self::dart::{
    constraints::{DisplayMediaStreamConstraints, MediaStreamConstraints},
    delay_for,
    error::Error,
    input_device_info::InputDeviceInfo,
    media_devices::{enumerate_devices, get_display_media, get_user_media},
    media_track::MediaStreamTrack,
    peer_connection::RtcPeerConnection,
    spawn,
    transceiver::Transceiver,
    utils::{callback::Callback, callback::Function},
};
#[cfg(feature = "wasm")]
pub use self::wasm::{
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
    transceiver::Transceiver,
    utils::{Callback, Function},
};

#[cfg(feature = "mockable")]
pub use self::transport::MockRpcTransport;

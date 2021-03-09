//! Platform-specific functionality.

pub mod peer_connection;
pub mod rtc_stats;
pub mod transport;
pub mod wasm;

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

#[cfg(feature = "mockable")]
pub use self::transport::MockRpcTransport;

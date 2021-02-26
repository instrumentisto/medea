//! Platform specific functionality.

#[doc(inline)]
pub use self::{
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
        peer_connection::{
            IceCandidate, RTCPeerConnectionError, RtcPeerConnection, SdpType,
        },
        rtc_stats::RtcStats,
        set_panic_hook, spawn,
        transceiver::{Transceiver, TransceiverDirection},
        utils::{Callback, Function},
    },
};

#[cfg(feature = "mockable")]
pub use self::transport::MockRpcTransport;

mod rtc_stats;
mod transport;
mod wasm;

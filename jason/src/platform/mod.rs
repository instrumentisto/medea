// #[cfg(all(
//     target_arch = "wasm32",
//     target_vendor = "unknown",
//     target_os = "unknown"
// ))]
mod wasm;

// #[cfg(all(
//     target_arch = "wasm32",
//     target_vendor = "unknown",
//     target_os = "unknown"
// ))]
pub use wasm::{
    delay_for, enumerate_devices, get_display_media, get_property_by_name,
    get_user_media, init_logger,
    input_device_info::InputDeviceInfo,
    media_track::MediaStreamTrack,
    peer_connection::{
        IceCandidate, RTCPeerConnectionError, RtcPeerConnection, SdpType,
    },
    rtc_stats::{RtcStats, RtcStatsError},
    set_panic_hook, spawn,
    transceiver::{Transceiver, TransceiverDirection},
    Error,
};

pub use web_sys::{
    ConstrainDomStringParameters, ConstrainDoubleRange,
    DisplayMediaStreamConstraints, MediaDeviceInfo, MediaDeviceKind,
    MediaStreamConstraints, MediaStreamTrackState, MediaTrackConstraints,
};

pub mod transport {
    use derive_more::Display;
    use futures::stream::LocalBoxStream;
    use medea_client_api_proto::{ClientMsg, ServerMsg};
    use tracerr::Traced;

    pub use super::wasm::transport::WebSocketRpcTransport;

    use crate::{
        core::{
            rpc::{ClientDisconnect, CloseMsg},
            utils::JsonParseError,
        },
        platform::{self, wasm::utils::EventListenerBindError},
        utils::JsCaused,
    };

    /// [`RpcTransport`] states.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum TransportState {
        /// Socket has been created. The connection is not open yet.
        ///
        /// Reflects `CONNECTING` state from JS side
        /// [`WebSocket.readyState`][1].
        ///
        /// [1]: https://developer.mozilla.org/docs/Web/API/WebSocket/readyState
        Connecting,

        /// The connection is open and ready to communicate.
        ///
        /// Reflects `OPEN` state from JS side [`WebSocket.readyState`][1].
        ///
        /// [1]: https://developer.mozilla.org/docs/Web/API/WebSocket/readyState
        Open,

        /// The connection is in the process of closing.
        ///
        /// Reflects `CLOSING` state from JS side [`WebSocket.readyState`][1].
        ///
        /// [1]: https://developer.mozilla.org/docs/Web/API/WebSocket/readyState
        Closing,

        /// The connection is closed or couldn't be opened.
        ///
        /// Reflects `CLOSED` state from JS side [`WebSocket.readyState`][1].
        ///
        /// [`CloseMsg`] is the reason of why [`RpcTransport`] went into
        /// this [`TransportState`].
        ///
        /// [1]: https://developer.mozilla.org/docs/Web/API/WebSocket/readyState
        Closed(CloseMsg),
    }

    impl TransportState {
        /// Returns `true` if socket can be closed.
        pub fn can_close(self) -> bool {
            matches!(self, Self::Connecting | Self::Open)
        }
    }

    /// RPC transport between a client and a server.
    #[cfg_attr(feature = "mockable", mockall::automock)]
    pub trait RpcTransport {
        /// Returns [`LocalBoxStream`] of all messages received by this
        /// transport.
        fn on_message(&self) -> LocalBoxStream<'static, ServerMsg>;

        /// Sets reason, that will be sent to remote server when this transport
        /// will be dropped.
        fn set_close_reason(&self, reason: ClientDisconnect);

        /// Sends given [`ClientMsg`] to a server.
        ///
        /// # Errors
        ///
        /// Errors if sending [`ClientMsg`] fails.
        fn send(&self, msg: &ClientMsg) -> Result<(), Traced<TransportError>>;

        /// Subscribes to a [`RpcTransport`]'s [`TransportState`] changes.
        fn on_state_change(&self) -> LocalBoxStream<'static, TransportState>;
    }

    /// Errors that may occur when working with [`WebSocketRpcClient`].
    ///
    /// [`WebSocketRpcClient`]: super::WebSocketRpcClient
    #[derive(Clone, Debug, Display, JsCaused, PartialEq)]
    #[js(error = "platform::Error")]
    pub enum TransportError {
        /// Occurs when the port to which the connection is being attempted
        /// is being blocked.
        #[display(fmt = "Failed to create WebSocket: {}", _0)]
        CreateSocket(platform::Error),

        /// Occurs when the connection close before becomes state active.
        #[display(fmt = "Failed to init WebSocket")]
        InitSocket,

        /// Occurs when [`ClientMsg`] cannot be parsed.
        #[display(fmt = "Failed to parse client message: {}", _0)]
        ParseClientMessage(JsonParseError),

        /// Occurs when [`ServerMsg`] cannot be parsed.
        #[display(fmt = "Failed to parse server message: {}", _0)]
        ParseServerMessage(JsonParseError),

        /// Occurs if the parsed message is not string.
        #[display(fmt = "Message is not a string")]
        MessageNotString,

        /// Occurs when a message cannot be send to server.
        #[display(fmt = "Failed to send message: {}", _0)]
        SendMessage(platform::Error),

        /// Occurs when handler failed to bind to some [WebSocket] event. Not
        /// really supposed to ever happen.
        ///
        /// [WebSocket]: https://developer.mozilla.org/ru/docs/WebSockets
        #[display(fmt = "Failed to bind to WebSocket event: {}", _0)]
        WebSocketEventBindError(EventListenerBindError), // TODO: remove

        /// Occurs when message is sent to a closed socket.
        #[display(fmt = "Underlying socket is closed")]
        ClosedSocket,
    }
}

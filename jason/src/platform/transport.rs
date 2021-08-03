//! Platform-agnostic functionality of RPC transport.

use derive_more::Display;
use futures::stream::LocalBoxStream;
use medea_client_api_proto::{ClientMsg, ServerMsg};
use tracerr::Traced;

use crate::{
    platform,
    rpc::{ClientDisconnect, CloseMsg},
    utils::{Caused, JsonParseError},
};

/// Possible states of a [`RpcTransport`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TransportState {
    /// Socket has been created. The connection is not opened yet.
    Connecting,

    /// The connection is opened and ready to communicate.
    Open,

    /// The connection is in the process of closing.
    Closing,

    /// The connection is closed or couldn't be opened.
    ///
    /// [`CloseMsg`] is the reason of why [`RpcTransport`] went into this
    /// [`TransportState`].
    Closed(CloseMsg),
}

impl TransportState {
    /// Indicates whether the socket can be closed.
    #[inline]
    #[must_use]
    pub fn can_close(self) -> bool {
        matches!(self, Self::Connecting | Self::Open)
    }
}

/// RPC transport between a client and a server.
#[cfg_attr(feature = "mockable", mockall::automock)]
pub trait RpcTransport {
    /// Returns [`LocalBoxStream`] of all messages received by this transport.
    fn on_message(&self) -> LocalBoxStream<'static, ServerMsg>;

    /// Sets reason, that will be sent to remote server when this transport will
    /// be dropped.
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

/// Errors that may occur when working with a [`RpcTransport`].
#[derive(Clone, Debug, Display, Caused, PartialEq)]
#[cause(error = "platform::Error")]
pub enum TransportError {
    /// Error encountered when trying to establish connection.
    #[display(fmt = "Failed to create WebSocket: {:?}", _0)]
    CreateSocket(platform::Error),

    /// Connection was closed before becoming active.
    #[display(fmt = "Failed to init WebSocket")]
    InitSocket,

    /// Occurs when [`ClientMsg`] cannot be serialized.
    #[display(fmt = "Failed to parse client message: {}", _0)]
    SerializeClientMessage(JsonParseError),

    /// Occurs when [`ServerMsg`] cannot be parsed.
    #[display(fmt = "Failed to parse server message: {}", _0)]
    ParseServerMessage(JsonParseError),

    /// Occurs if the parsed message is not string.
    #[display(fmt = "Message is not a string")]
    MessageNotString,

    /// Occurs when a message cannot be send to server.
    #[display(fmt = "Failed to send message: {:?}", _0)]
    SendMessage(platform::Error),

    /// Occurs when message is sent to a closed socket.
    #[display(fmt = "Underlying socket is closed")]
    ClosedSocket,
}

//! [WebSocket] transport wrapper.
//!
//! [WebSocket]: https://developer.mozilla.org/ru/docs/WebSockets

use futures::stream::LocalBoxStream;
use medea_client_api_proto::{ClientMsg, ServerMsg};
use tracerr::Traced;

use crate::{
    platform::transport::{RpcTransport, TransportError, TransportState},
    rpc::{websocket::ClientDisconnect, ApiUrl},
};

type Result<T, E = Traced<TransportError>> = std::result::Result<T, E>;

/// WebSocket [`RpcTransport`] between a client and a server.
///
/// # Drop
///
/// This structure has __cyclic references__, which are freed in its [`Drop`]
/// implementation.
///
/// If you're adding new cyclic dependencies, then don't forget to drop them in
/// the [`Drop`].
pub struct WebSocketRpcTransport;

impl WebSocketRpcTransport {
    /// Initiates new WebSocket connection. Resolves only when underlying
    /// connection becomes active.
    ///
    /// # Errors
    ///
    /// With [`TransportError::CreateSocket`] if cannot establish WebSocket to
    /// specified URL.
    ///
    /// With [`TransportError::InitSocket`] if [WebSocket.onclose][1] callback
    /// fired before [WebSocket.onopen][2] callback.
    ///
    /// [1]: https://developer.mozilla.org/en-US/docs/Web/API/WebSocket/onclose
    /// [2]: https://developer.mozilla.org/en-US/docs/Web/API/WebSocket/onopen
    pub async fn new(url: ApiUrl) -> Result<Self> {
        unimplemented!()
    }
}

impl RpcTransport for WebSocketRpcTransport {
    #[inline]
    fn on_message(&self) -> LocalBoxStream<'static, ServerMsg> {
        unimplemented!()
    }

    #[inline]
    fn set_close_reason(&self, close_reason: ClientDisconnect) {
        unimplemented!()
    }

    fn send(&self, msg: &ClientMsg) -> Result<()> {
        unimplemented!()
    }

    #[inline]
    fn on_state_change(&self) -> LocalBoxStream<'static, TransportState> {
        unimplemented!()
    }
}

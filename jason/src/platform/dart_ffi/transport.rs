//! [WebSocket] transport wrapper.
//!
//! [WebSocket]: https://developer.mozilla.org/ru/docs/WebSockets

use std::{cell::RefCell, convert::TryFrom, rc::Rc};

use derive_more::{From, Into};
use futures::stream::LocalBoxStream;
use medea_client_api_proto::{ClientMsg, ServerMsg};
use tracerr::Traced;
use web_sys::{CloseEvent, MessageEvent};

use crate::{
    platform::transport::{RpcTransport, TransportError, TransportState},
    rpc::{websocket::ClientDisconnect, ApiUrl, CloseMsg},
};

/// Wrapper for help to get [`ServerMsg`] from Websocket [MessageEvent][1].
///
/// [1]: https://developer.mozilla.org/en-US/docs/Web/API/MessageEvent
#[derive(Clone, From, Into)]
struct ServerMessage(ServerMsg);

impl TryFrom<&MessageEvent> for ServerMessage {
    type Error = TransportError;

    fn try_from(msg: &MessageEvent) -> std::result::Result<Self, Self::Error> {
        unimplemented!()
    }
}

type Result<T, E = Traced<TransportError>> = std::result::Result<T, E>;

struct InnerSocket {}

/// WebSocket [`RpcTransport`] between a client and a server.
///
/// # Drop
///
/// This structure has __cyclic references__, which are freed in its [`Drop`]
/// implementation.
///
/// If you're adding new cyclic dependencies, then don't forget to drop them in
/// the [`Drop`].
pub struct WebSocketRpcTransport(Rc<RefCell<InnerSocket>>);

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
    /// # Panics
    ///
    /// If binding to the [`close`][3] or the [`open`][4] events fails. Not
    /// supposed to ever happen.
    ///
    /// [1]: https://developer.mozilla.org/en-US/docs/Web/API/WebSocket/onclose
    /// [2]: https://developer.mozilla.org/en-US/docs/Web/API/WebSocket/onopen
    /// [3]: https://html.spec.whatwg.org/#event-close
    /// [4]: https://html.spec.whatwg.org/#event-open
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

impl From<&CloseEvent> for CloseMsg {
    fn from(event: &CloseEvent) -> Self {
        unimplemented!()
    }
}

//! [WebSocket] transport wrapper.
//!
//! [WebSocket]: https://developer.mozilla.org/ru/docs/WebSockets

use std::{cell::RefCell, convert::TryFrom, rc::Rc};

use derive_more::{From, Into};
use futures::{channel::mpsc, stream::LocalBoxStream, StreamExt};
use medea_client_api_proto::{ClientMsg, CloseDescription, ServerMsg};
use medea_reactive::ObservableCell;
use tracerr::Traced;
use web_sys::{CloseEvent, Event, MessageEvent, WebSocket as SysWebSocket};

use crate::{
    platform::{
        transport::{RpcTransport, TransportError, TransportState},
        wasm::utils::EventListener,
    },
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
        use TransportError::{MessageNotString, ParseServerMessage};

        let payload = msg.data().as_string().ok_or(MessageNotString)?;

        serde_json::from_str::<ServerMsg>(&payload)
            .map_err(|e| ParseServerMessage(e.into()))
            .map(Self::from)
    }
}

type Result<T, E = Traced<TransportError>> = std::result::Result<T, E>;

struct InnerSocket {
    /// JS side [WebSocket].
    ///
    /// [WebSocket]: https://developer.mozilla.org/docs/Web/API/WebSocket
    socket: Rc<SysWebSocket>,

    /// State of [`WebSocketRpcTransport`] connection.
    socket_state: ObservableCell<TransportState>,

    /// Listener for [WebSocket] [open event][1].
    ///
    /// [WebSocket]: https://developer.mozilla.org/docs/Web/API/WebSocket
    /// [1]: https://developer.mozilla.org/en-US/Web/API/WebSocket/open_event
    on_open_listener: Option<EventListener<SysWebSocket, Event>>,

    /// Listener for [WebSocket] [message event][1].
    ///
    /// [WebSocket]: https://developer.mozilla.org/docs/Web/API/WebSocket
    /// [1]: https://developer.mozilla.org/docs/Web/API/WebSocket/message_event
    on_message_listener: Option<EventListener<SysWebSocket, MessageEvent>>,

    /// Listener for [WebSocket] [close event][1].
    ///
    /// [WebSocket]: https://developer.mozilla.org/docs/Web/API/WebSocket
    /// [1]: https://developer.mozilla.org/docs/Web/API/WebSocket/close_event
    on_close_listener: Option<EventListener<SysWebSocket, CloseEvent>>,

    /// Subscribers for [`RpcTransport::on_message`] events.
    on_message_subs: Vec<mpsc::UnboundedSender<ServerMsg>>,

    /// Reason of [`WebSocketRpcTransport`] closing.
    /// Will be sent in [WebSocket close frame][1].
    ///
    /// [1]: https://tools.ietf.org/html/rfc6455#section-5.5.1
    close_reason: ClientDisconnect,
}

impl InnerSocket {
    fn new(url: &str) -> Result<Self> {
        let socket = SysWebSocket::new(&url)
            .map_err(Into::into)
            .map_err(TransportError::CreateSocket)
            .map_err(tracerr::wrap!())?;
        Ok(Self {
            socket_state: ObservableCell::new(TransportState::Connecting),
            socket: Rc::new(socket),
            on_open_listener: None,
            on_message_listener: None,
            on_close_listener: None,
            on_message_subs: Vec::new(),
            close_reason: ClientDisconnect::RpcTransportUnexpectedlyDropped,
        })
    }
}

impl Drop for InnerSocket {
    fn drop(&mut self) {
        if self.socket_state.borrow().can_close() {
            let rsn = serde_json::to_string(&self.close_reason)
                .expect("Could not serialize close message");
            if let Err(e) = self.socket.close_with_code_and_reason(1000, &rsn) {
                log::error!("Failed to normally close socket: {:?}", e);
            }
        }
    }
}

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
        let socket = Rc::new(RefCell::new(InnerSocket::new(url.as_ref())?));
        {
            let mut socket_mut = socket.borrow_mut();
            let inner = Rc::clone(&socket);
            socket_mut.on_close_listener = Some(
                EventListener::new_once(
                    Rc::clone(&socket_mut.socket),
                    "close",
                    move |msg: CloseEvent| {
                        inner
                            .borrow()
                            .socket_state
                            .set(TransportState::Closed(CloseMsg::from(&msg)));
                    },
                )
                .unwrap(),
            );

            let inner = Rc::clone(&socket);
            socket_mut.on_open_listener = Some(
                EventListener::new_once(
                    Rc::clone(&socket_mut.socket),
                    "open",
                    move |_| {
                        inner.borrow().socket_state.set(TransportState::Open);
                    },
                )
                .unwrap(),
            );
        }

        let state_updates_rx = socket.borrow().socket_state.subscribe();
        let state = state_updates_rx.skip(1).next().await;

        if let Some(TransportState::Open) = state {
            let this = Self(socket);
            this.set_on_close_listener();
            this.set_on_message_listener();

            Ok(this)
        } else {
            Err(tracerr::new!(TransportError::InitSocket))
        }
    }

    /// Sets [`InnerSocket::on_close_listener`] which will update
    /// [`RpcTransport`]'s [`TransportState`] to [`TransportState::Closed`].
    fn set_on_close_listener(&self) {
        let this = Rc::clone(&self.0);
        let on_close = EventListener::new_once(
            Rc::clone(&self.0.borrow().socket),
            "close",
            move |msg: CloseEvent| {
                this.borrow()
                    .socket_state
                    .set(TransportState::Closed(CloseMsg::from(&msg)));
            },
        )
        .unwrap();
        self.0.borrow_mut().on_close_listener = Some(on_close);
    }

    /// Sets [`InnerSocket::on_message_listener`] which will send
    /// [`ServerMessage`]s to [`WebSocketRpcTransport::on_message`] subscribers.
    fn set_on_message_listener(&self) {
        let this = Rc::clone(&self.0);
        let on_message = EventListener::new_mut(
            Rc::clone(&self.0.borrow().socket),
            "message",
            move |msg| {
                let msg =
                    match ServerMessage::try_from(&msg).map(ServerMsg::from) {
                        Ok(parsed) => parsed,
                        Err(e) => {
                            // TODO: protocol versions mismatch? should drop
                            //       connection if so
                            log::error!("{}", tracerr::new!(e));
                            return;
                        }
                    };

                let mut this_mut = this.borrow_mut();
                this_mut.on_message_subs.retain(|on_message| {
                    on_message.unbounded_send(msg.clone()).is_ok()
                });
            },
        )
        .unwrap();

        self.0.borrow_mut().on_message_listener = Some(on_message);
    }
}

impl RpcTransport for WebSocketRpcTransport {
    #[inline]
    fn on_message(&self) -> LocalBoxStream<'static, ServerMsg> {
        let (tx, rx) = mpsc::unbounded();
        self.0.borrow_mut().on_message_subs.push(tx);

        Box::pin(rx)
    }

    #[inline]
    fn set_close_reason(&self, close_reason: ClientDisconnect) {
        self.0.borrow_mut().close_reason = close_reason;
    }

    fn send(&self, msg: &ClientMsg) -> Result<()> {
        let inner = self.0.borrow();
        let message = serde_json::to_string(msg)
            .map_err(|e| TransportError::ParseClientMessage(e.into()))
            .map_err(tracerr::wrap!())?;

        let state = &*inner.socket_state.borrow();
        match state {
            TransportState::Open => inner
                .socket
                .send_with_str(&message)
                .map_err(Into::into)
                .map_err(TransportError::SendMessage)
                .map_err(tracerr::wrap!()),
            _ => Err(tracerr::new!(TransportError::ClosedSocket)),
        }
    }

    #[inline]
    fn on_state_change(&self) -> LocalBoxStream<'static, TransportState> {
        self.0.borrow().socket_state.subscribe()
    }
}

impl Drop for WebSocketRpcTransport {
    /// Don't forget that [`WebSocketRpcTransport`] is a [`Rc`] and this
    /// [`Drop`] implementation will be called on each drop of its references.
    fn drop(&mut self) {
        let mut inner = self.0.borrow_mut();
        inner.on_open_listener.take();
        inner.on_message_listener.take();
        inner.on_close_listener.take();
    }
}

impl From<&CloseEvent> for CloseMsg {
    fn from(event: &CloseEvent) -> Self {
        let code: u16 = event.code();
        match code {
            1000 => {
                if let Ok(description) =
                    serde_json::from_str::<CloseDescription>(&event.reason())
                {
                    Self::Normal(code, description.reason)
                } else {
                    Self::Abnormal(code)
                }
            }
            _ => Self::Abnormal(code),
        }
    }
}

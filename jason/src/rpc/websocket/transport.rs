//! [WebSocket] transport wrapper.
//!
//! [WebSocket]: https://developer.mozilla.org/ru/docs/WebSockets

use std::{cell::RefCell, convert::TryFrom, rc::Rc};

use derive_more::{Display, From, Into};
use futures::{channel::mpsc, stream::LocalBoxStream, StreamExt};
use medea_client_api_proto::{ClientMsg, ServerMsg};
use medea_reactive::ObservableCell;
use tracerr::Traced;
use web_sys::{CloseEvent, Event, MessageEvent, WebSocket as SysWebSocket};

use crate::{
    rpc::{websocket::client::ClientDisconnect, ApiUrl, CloseMsg},
    utils::{
        EventListener, EventListenerBindError, JasonError, JsCaused, JsError,
        JsonParseError,
    },
};

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

/// Errors that may occur when working with [`WebSocketRpcClient`].
///
/// [`WebSocketRpcClient`]: super::WebSocketRpcClient
#[derive(Clone, Debug, Display, JsCaused, PartialEq)]
pub enum TransportError {
    /// Occurs when the port to which the connection is being attempted
    /// is being blocked.
    #[display(fmt = "Failed to create WebSocket: {}", _0)]
    CreateSocket(JsError),

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
    SendMessage(JsError),

    /// Occurs when handler failed to bind to some [WebSocket] event. Not
    /// really supposed to ever happen.
    ///
    /// [WebSocket]: https://developer.mozilla.org/ru/docs/WebSockets
    #[display(fmt = "Failed to bind to WebSocket event: {}", _0)]
    WebSocketEventBindError(EventListenerBindError),

    /// Occurs when message is sent to a closed socket.
    #[display(fmt = "Underlying socket is closed")]
    ClosedSocket,
}

impl From<EventListenerBindError> for TransportError {
    fn from(err: EventListenerBindError) -> Self {
        Self::WebSocketEventBindError(err)
    }
}

/// Wrapper for help to get [`ServerMsg`] from Websocket [MessageEvent][1].
///
/// [1]: https://developer.mozilla.org/en-US/docs/Web/API/MessageEvent
#[derive(Clone, From, Into)]
pub struct ServerMessage(ServerMsg);

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

/// [`RpcTransport`] states.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TransportState {
    /// Socket has been created. The connection is not open yet.
    ///
    /// Reflects `CONNECTING` state from JS side [`WebSocket.readyState`][1].
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
    #[inline]
    #[must_use]
    pub fn can_close(self) -> bool {
        matches!(self, Self::Connecting | Self::Open)
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
    /// [1]: https://developer.mozilla.org/en-US/docs/Web/API/WebSocket/onclose
    /// [2]: https://developer.mozilla.org/en-US/docs/Web/API/WebSocket/onopen
    pub async fn new(url: ApiUrl) -> Result<Self> {
        let socket = Rc::new(RefCell::new(InnerSocket::new(url.0.as_str())?));
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
                .map_err(tracerr::map_from_and_wrap!())?,
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
                .map_err(tracerr::map_from_and_wrap!(=> TransportError))?,
            );
        }

        let state_updates_rx = socket.borrow().socket_state.subscribe();
        let state = state_updates_rx.skip(1).next().await;

        if let Some(TransportState::Open) = state {
            let this = Self(socket);
            this.set_on_close_listener()?;
            this.set_on_message_listener()?;

            Ok(this)
        } else {
            Err(tracerr::new!(TransportError::InitSocket))
        }
    }

    /// Sets [`InnerSocket::on_close_listener`] which will update
    /// [`RpcTransport`]'s [`TransportState`] to [`TransportState::Closed`].
    fn set_on_close_listener(&self) -> Result<()> {
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
        .map_err(tracerr::map_from_and_wrap!(=> TransportError))?;
        self.0.borrow_mut().on_close_listener = Some(on_close);

        Ok(())
    }

    /// Sets [`InnerSocket::on_message_listener`] which will send
    /// [`ServerMessage`]s to [`WebSocketRpcTransport::on_message`] subscribers.
    fn set_on_message_listener(&self) -> Result<()> {
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
                            JasonError::from(tracerr::new!(e)).print();
                            return;
                        }
                    };

                let mut this_mut = this.borrow_mut();
                this_mut.on_message_subs.retain(|on_message| {
                    on_message.unbounded_send(msg.clone()).is_ok()
                });
            },
        )
        .map_err(tracerr::map_from_and_wrap!(=> TransportError))?;

        self.0.borrow_mut().on_message_listener = Some(on_message);

        Ok(())
    }
}

impl RpcTransport for WebSocketRpcTransport {
    fn on_message(&self) -> LocalBoxStream<'static, ServerMsg> {
        let (tx, rx) = mpsc::unbounded();
        self.0.borrow_mut().on_message_subs.push(tx);

        Box::pin(rx)
    }

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

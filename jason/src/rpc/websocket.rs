//! [WebSocket] transport wrapper.
//!
//! [WebSocket]: https://developer.mozilla.org/ru/docs/WebSockets

use std::{borrow::Cow, cell::RefCell, convert::TryFrom, rc::Rc};

use derive_more::{Display, From, Into};
use futures::{
    channel::{mpsc, oneshot},
    future,
    stream::LocalBoxStream,
};
use medea_client_api_proto::{ClientMsg, ServerMsg};
use tracerr::Traced;
use web_sys::{CloseEvent, Event, MessageEvent, WebSocket as SysWebSocket};

use crate::utils::{
    console_error, EventListener, EventListenerBindError, JasonError, JsCaused,
    JsError,
};

use super::{
    ClientDisconnect, CloseMsg, ClosedStateReason, RpcTransport, State,
};

/// Errors that may occur when working with [`WebSocket`].
#[derive(Clone, Debug, Display, JsCaused)]
pub enum TransportError {
    /// Occurs when the port to which the connection is being attempted
    /// is being blocked.
    #[display(fmt = "Failed to create WebSocket: {}", _0)]
    CreateSocket(JsError),

    /// Occurs when the connection close before becomes state active.
    #[display(fmt = "Failed to init WebSocket")]
    InitSocket,

    /// Occurs when [`ClientMessage`] cannot be parsed.
    #[display(fmt = "Failed to parse client message: {}", _0)]
    ParseClientMessage(String),

    /// Occurs when [`ServerMessage`] cannot be parsed.
    #[display(fmt = "Failed to parse server message: {}", _0)]
    ParseServerMessage(String),

    /// Occurs if the parsed message is not string.
    #[display(fmt = "Message is not a string")]
    MessageNotString,

    /// Occurs when a message cannot be send to server.
    #[display(fmt = "Failed to send message: {}", _0)]
    SendMessage(JsError),

    /// Occurs when handler failed to bind to some [`WebSocket`] event. Not
    /// really supposed to ever happen.
    #[display(fmt = "Failed to bind to WebSocket event: {}", _0)]
    WebSocketEventBindError(EventListenerBindError),

    /// Occurs when message is sent to a closed socket.
    #[display(fmt = "Underlying socket is closed")]
    ClosedSocket,
}

/// Wrapper for help to get [`ServerMsg`] from Websocket [MessageEvent][1].
///
/// [1]: https://developer.mozilla.org/en-US/docs/Web/API/MessageEvent
#[derive(Clone, From, Into)]
pub struct ServerMessage(ServerMsg);

impl TryFrom<&MessageEvent> for ServerMessage {
    type Error = TransportError;

    fn try_from(msg: &MessageEvent) -> std::result::Result<Self, Self::Error> {
        use TransportError::*;
        let payload = msg.data().as_string().ok_or(MessageNotString)?;

        serde_json::from_str::<ServerMsg>(&payload)
            .map_err(|e| e.to_string())
            .map_err(ParseServerMessage)
            .map(Self::from)
    }
}

impl From<EventListenerBindError> for TransportError {
    fn from(err: EventListenerBindError) -> Self {
        Self::WebSocketEventBindError(err)
    }
}

type Result<T, E = Traced<TransportError>> = std::result::Result<T, E>;

struct InnerSocket {
    /// JS side [`WebSocket`].
    ///
    /// [`WebSocket`]:
    /// https://developer.mozilla.org/en-US/docs/Web/API/WebSocket
    socket: Rc<SysWebSocket>,

    /// State of [`WebSocketTransport`] connection.
    socket_state: State,

    /// Listener for [`WebSocket`] [`open`] event.
    ///
    /// [`WebSocket`]:
    /// https://developer.mozilla.org/en-US/docs/Web/API/WebSocket
    /// [`open`]:
    /// https://developer.mozilla.org/en-US/docs/Web/API/WebSocket/open_event
    on_open_listener: Option<EventListener<SysWebSocket, Event>>,

    /// Listener for [`WebSocket`] [`message`] event.
    ///
    /// [`WebSocket`]:
    /// https://developer.mozilla.org/en-US/docs/Web/API/WebSocket
    /// [`message`]:
    /// https://developer.mozilla.org/en-US/docs/Web/API/WebSocket/message_event
    on_message_listener: Option<EventListener<SysWebSocket, MessageEvent>>,

    /// Listener for [`WebSocket`] [`close`] event.
    ///
    /// [`WebSocket`]:
    /// https://developer.mozilla.org/en-US/docs/Web/API/WebSocket
    /// [`close`]:
    /// https://developer.mozilla.org/en-US/docs/Web/API/WebSocket/close_event
    on_close_listener: Option<EventListener<SysWebSocket, CloseEvent>>,

    /// [`mpsc::UnboundedSender`]s for [`RpcTransport::on_message`]'s
    /// [`LocalBoxStream`].
    on_message_subs: Vec<mpsc::UnboundedSender<ServerMsg>>,

    /// [mpsc::UnboundedSender`]s for [`RpcTransport::on_state_change`]'s
    /// [`LocalBoxStream`].
    on_state_change_subs: Vec<mpsc::UnboundedSender<State>>,

    /// Reason of [`WebSocketRpcTransport`] closing. Will be sent in
    /// `WebSocket` [close frame].
    ///
    /// [close frame]:
    /// https://tools.ietf.org/html/rfc6455#section-5.5.1
    close_reason: ClientDisconnect,
}

/// WebSocket [`RpcTransport`] between a client and server.
///
/// Don't derive [`Clone`] and don't use it if there are no very serious reasons
/// for this. Because with many strong [`Rc`]s we can catch many painful bugs
/// with [`Drop`] implementation, memory leaks etc. It is especially not
/// recommended use a strong pointer ([`Rc`]) in all kinds of callbacks and
/// `async` closures. If you clone this then make sure that this
/// [`WebSocketRpcTransport`] will be normally [`Drop`]ed.
///
/// Alternative for [`Clone`] is [`WebSocketRpcTransport::downgrade`] which will
/// return [`WeakWebSocketRpcTransport`] which can be upgraded to
/// [`WebSocketRpcTransport`] and will not hold this structure from destruction.
pub struct WebSocketRpcTransport(Rc<RefCell<InnerSocket>>);

impl InnerSocket {
    fn new(url: &str) -> Result<Self> {
        let socket = SysWebSocket::new(&url)
            .map_err(Into::into)
            .map_err(TransportError::CreateSocket)
            .map_err(tracerr::wrap!())?;
        Ok(Self {
            socket_state: State::Connecting,
            socket: Rc::new(socket),
            on_open_listener: None,
            on_message_listener: None,
            on_close_listener: None,
            on_message_subs: Vec::new(),
            on_state_change_subs: Vec::new(),
            close_reason: ClientDisconnect::RpcTransportUnexpectedlyDropped,
        })
    }

    /// Updates `socket_state` with provided [`State`].
    ///
    /// Sends updated [`State`] to the `on_state_change` subscribers. But
    /// if [`State`] is not changed, nothing will be sent.
    fn update_socket_state(&mut self, new_state: &State) {
        if self.socket_state.id() != new_state.id() {
            self.socket_state = new_state.clone();
            self.on_state_change_subs.retain(|sub| !sub.is_closed());

            self.on_state_change_subs
                .iter()
                .filter_map(|sub| sub.unbounded_send(new_state.clone()).err())
                .for_each(|e| {
                    console_error(format!(
                        "'WebSocketRpcTransport::on_state_change' subscriber \
                         unexpectedly gone. {:?}",
                        e
                    ));
                });
        }
    }

    /// Checks underlying WebSocket state and updates `socket_state`.
    fn sync_socket_state(&mut self) {
        self.update_socket_state(&self.socket.ready_state().into());
    }
}

impl RpcTransport for WebSocketRpcTransport {
    fn on_message(&self) -> Result<LocalBoxStream<'static, ServerMsg>> {
        let (tx, rx) = mpsc::unbounded();
        self.0.borrow_mut().on_message_subs.push(tx);

        Ok(Box::pin(rx))
    }

    fn send(&self, msg: &ClientMsg) -> Result<()> {
        let inner = self.0.borrow();
        let message = serde_json::to_string(msg)
            .map_err(|e| e.to_string())
            .map_err(TransportError::ParseClientMessage)
            .map_err(tracerr::wrap!())?;

        match inner.socket_state {
            State::Open => inner
                .socket
                .send_with_str(&message)
                .map_err(Into::into)
                .map_err(TransportError::SendMessage)
                .map_err(tracerr::wrap!()),
            _ => Err(tracerr::new!(TransportError::ClosedSocket)),
        }
    }

    fn set_close_reason(&self, close_reason: ClientDisconnect) {
        self.0.borrow_mut().close_reason = close_reason;
    }

    fn on_state_change(&self) -> LocalBoxStream<'static, State> {
        let (tx, rx) = mpsc::unbounded();
        self.0.borrow_mut().on_state_change_subs.push(tx);

        Box::pin(rx)
    }
}

impl WebSocketRpcTransport {
    /// Initiates new WebSocket connection. Resolves only when underlying
    /// connection becomes active.
    pub async fn new(url: &str) -> Result<Self> {
        let (tx_close, rx_close) = oneshot::channel();
        let (tx_open, rx_open) = oneshot::channel();

        let inner = InnerSocket::new(url)?;
        let socket = Self(Rc::new(RefCell::new(inner)));

        {
            let inner = Rc::downgrade(&socket.0);
            let socket_transport = socket.0.borrow().socket.clone();
            socket.0.borrow_mut().on_close_listener = Some(
                EventListener::new_once(
                    Rc::clone(&socket_transport),
                    "close",
                    move |_| {
                        if let Some(inner) = inner.upgrade().map(Self) {
                            inner.0.borrow_mut().sync_socket_state();
                        }
                        let _ = tx_close.send(());
                    },
                )
                .map_err(tracerr::map_from_and_wrap!())?,
            );

            let inner = Rc::downgrade(&socket.0);
            socket.0.borrow_mut().on_open_listener = Some(
                EventListener::new_once(
                    Rc::clone(&socket_transport),
                    "open",
                    move |_| {
                        if let Some(inner) = inner.upgrade().map(Self) {
                            inner.0.borrow_mut().sync_socket_state();
                        }
                        let _ = tx_open.send(());
                    },
                )
                .map_err(tracerr::map_from_and_wrap!(=> TransportError))?,
            );
        }

        let state = future::select(rx_open, rx_close).await;

        socket.0.borrow_mut().on_open_listener.take();
        socket.0.borrow_mut().on_close_listener.take();
        socket.set_on_close_listener()?;
        socket.set_on_message_listener()?;

        match state {
            future::Either::Left((opened, _)) => match opened {
                Ok(_) => Ok(socket),
                Err(_) => Err(tracerr::new!(TransportError::InitSocket)),
            },
            future::Either::Right(_closed) => {
                Err(tracerr::new!(TransportError::InitSocket))
            }
        }
    }

    /// Sets [`WebSocketRpcTransport::on_close_listener`] which will send
    /// [`CloseMsg`]s to [`WebSocketRpcTransport::on_close`].
    fn set_on_close_listener(&self) -> Result<()> {
        let weak_transport = Rc::downgrade(&self.0);
        let on_close = EventListener::new_once(
            Rc::clone(&self.0.borrow().socket),
            "close",
            move |msg: CloseEvent| {
                let close_msg = CloseMsg::from(&msg);
                let transport = if let Some(socket_clone) =
                    weak_transport.upgrade().map(Self)
                {
                    socket_clone
                } else {
                    console_error(
                        "'WebSocketRpcTransport' was unexpectedly gone.",
                    );
                    return;
                };
                transport.0.borrow_mut().update_socket_state(&State::Closed(
                    ClosedStateReason::ConnectionLost(close_msg),
                ));
            },
        )
        .map_err(tracerr::map_from_and_wrap!(=> TransportError))?;
        self.0.borrow_mut().on_close_listener = Some(on_close);

        Ok(())
    }

    /// Sets [`WebSocketRpcTransport::on_message_listener`] which will send
    /// [`ServerMessage`]s to [`WebSocketRpcTransport::on_message`].
    fn set_on_message_listener(&self) -> Result<()> {
        let weak_transport = Rc::downgrade(&self.0);
        let on_message = EventListener::new_mut(
            Rc::clone(&self.0.borrow().socket),
            "message",
            move |msg| {
                let parsed: ServerMsg =
                    match ServerMessage::try_from(&msg).map(Into::into) {
                        Ok(parsed) => parsed,
                        Err(e) => {
                            // TODO: protocol versions mismatch? should drop
                            //       connection if so
                            JasonError::from(tracerr::new!(e)).print();
                            return;
                        }
                    };
                let transport = if let Some(transport) =
                    weak_transport.upgrade().map(Self)
                {
                    transport
                } else {
                    console_error(
                        "'WebSocketRpcTransport' was unexpectedly gone.",
                    );
                    return;
                };
                let mut transport_ref = transport.0.borrow_mut();
                transport_ref
                    .on_message_subs
                    .retain(|on_message| !on_message.is_closed());
                transport_ref.on_message_subs.iter().for_each(|on_message| {
                    on_message.unbounded_send(parsed.clone()).unwrap_or_else(
                        |e| {
                            console_error(format!(
                                "WebSocket's 'on_message' callback receiver \
                                 unexpectedly gone. {:?}",
                                e
                            ))
                        },
                    );
                })
            },
        )
        .map_err(tracerr::map_from_and_wrap!(=> TransportError))?;

        self.0.borrow_mut().on_message_listener = Some(on_message);

        Ok(())
    }
}

impl Drop for InnerSocket {
    fn drop(&mut self) {
        if self.socket_state.can_close() {
            let close_reason: Cow<'static, str> =
                serde_json::to_string(&self.close_reason)
                    .unwrap_or_else(|_| {
                        "Could not serialize close message".into()
                    })
                    .into();

            if let Err(err) =
                self.socket.close_with_code_and_reason(1000, &close_reason)
            {
                console_error(err);
            }
        }
    }
}

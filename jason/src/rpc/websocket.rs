//! [WebSocket] transport wrapper.
//!
//! [WebSocket]: https://developer.mozilla.org/ru/docs/WebSockets

use std::{
    borrow::Cow,
    cell::RefCell,
    convert::TryFrom,
    rc::{Rc, Weak},
};

use derive_more::{Display, From, Into};
use futures::{
    channel::{mpsc, oneshot},
    future::{self, LocalBoxFuture},
    stream::LocalBoxStream,
};
use medea_client_api_proto::{ClientMsg, ServerMsg};
use tracerr::Traced;
use web_sys::{CloseEvent, Event, MessageEvent, WebSocket as SysWebSocket};

use crate::{
    rpc::{ClientDisconnect, CloseMsg, RpcTransport},
    utils::{
        console_error, EventListener, EventListenerBindError, JasonError,
        JsCaused, JsError,
    },
};

/// Errors that may occur when working with [`WebSocket`].
#[derive(Debug, Display, JsCaused)]
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
    ParseClientMessage(serde_json::error::Error),

    /// Occurs when [`ServerMessage`] cannot be parsed.
    #[display(fmt = "Failed to parse server message: {}", _0)]
    ParseServerMessage(serde_json::error::Error),

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

/// State of WebSocket.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// Socket has been created. The connection is not yet open.
    ///
    /// Reflects `CONNECTING` state from JS side [`WebSocket.readyState`].
    ///
    /// [`WebSocket.readyState`]: https://tinyurl.com/t8ovwvr
    Connecting,

    /// The connection is open and ready to communicate.
    ///
    /// Reflects `OPEN` state from JS side [`WebSocket.readyState`].
    ///
    /// [`WebSocket.readyState`]: https://tinyurl.com/t8ovwvr
    Open,

    /// The connection is in the process of closing.
    ///
    /// Reflects `CLOSING` state from JS side [`WebSocket.readyState`].
    ///
    /// [`WebSocket.readyState`]: https://tinyurl.com/t8ovwvr
    Closing,

    /// The connection is closed or couldn't be opened.
    ///
    /// Reflects `CLOSED` state from JS side [`WebSocket.readyState`].
    ///
    /// [`WebSocket.readyState`]: https://tinyurl.com/t8ovwvr
    Closed,
}

impl State {
    /// Returns `true` if socket can be closed.
    pub fn can_close(self) -> bool {
        match self {
            Self::Connecting | Self::Open => true,
            _ => false,
        }
    }
}

impl From<u16> for State {
    fn from(value: u16) -> Self {
        match value {
            0 => Self::Connecting,
            1 => Self::Open,
            2 => Self::Closing,
            3 => Self::Closed,
            _ => unreachable!(),
        }
    }
}

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

    // TODO: this already looks kinda wrong, if we have `on_state_change`,
    //       then why we need explicit `on_close`?

    /// [`mpsc::UnboundedSender`]s for [`RpcTransport::on_close`]'s
    /// [`LocalBoxStream`].
    on_close_subs: Vec<mpsc::UnboundedSender<CloseMsg>>,

    /// [mpsc::UnboundedSender`]s for [`RpcTransport::on_state_change`]'s
    /// [`LocalBoxStream`].
    on_state_change_subs: Vec<mpsc::UnboundedSender<State>>,

    /// Reason of [`WebSocketRpcTransport`] closing. Will be sent in
    /// `WebSocket` [close frame].
    ///
    /// [close frame]:
    /// https://tools.ietf.org/html/rfc6455#section-5.5.1
    close_reason: ClientDisconnect,

    /// URL to which this [`WebSocketRpcTransport`] is connected.
    url: String,
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
    fn new(url: String) -> Result<Self> {
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
            on_close_subs: Vec::new(),
            on_message_subs: Vec::new(),
            on_state_change_subs: Vec::new(),
            close_reason: ClientDisconnect::RpcTransportUnexpectedlyDropped,
            url,
        })
    }

    /// Updates `socket_state` with provided [`State`].
    ///
    /// Sends updated [`State`] to the `on_state_change` subscribers. But
    /// if [`State`] is not changed, nothing will be sent.
    fn update_socket_state(&mut self, new_state: State) {
        if self.socket_state != new_state {
            self.socket_state = new_state;

            self.on_state_change_subs.retain(|sub| !sub.is_closed());

            self.on_state_change_subs
                .iter()
                .filter_map(|sub| sub.unbounded_send(new_state).err())
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
        self.update_socket_state(self.socket.ready_state().into());
    }
}

impl RpcTransport for WebSocketRpcTransport {
    fn on_message(&self) -> Result<LocalBoxStream<'static, ServerMsg>> {
        let (tx, rx) = mpsc::unbounded();
        self.0.borrow_mut().on_message_subs.push(tx);

        Ok(Box::pin(rx))
    }

    fn on_close(&self) -> Result<LocalBoxStream<'static, CloseMsg>> {
        let (tx, rx) = mpsc::unbounded();
        self.0.borrow_mut().on_close_subs.push(tx);

        Ok(Box::pin(rx))
    }

    fn send(&self, msg: &ClientMsg) -> Result<()> {
        let inner = self.0.borrow();
        let message = serde_json::to_string(msg)
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

    fn reconnect(&self) -> LocalBoxFuture<'static, Result<()>> {
        let this = Self(Rc::clone(&self.0));
        Box::pin(async move {
            let url = this.0.borrow().url.clone();
            this.0.borrow_mut().update_socket_state(State::Connecting);
            let new_transport = Self::new(url).await.map_err(|e| {
                this.0.borrow_mut().sync_socket_state();
                tracerr::new!(e)
            })?;

            std::mem::swap(
                &mut new_transport.0.borrow_mut().on_message_subs,
                &mut this.0.borrow_mut().on_message_subs,
            );
            std::mem::swap(
                &mut new_transport.0.borrow_mut().on_close_subs,
                &mut this.0.borrow_mut().on_close_subs,
            );

            RefCell::swap(&this.0, &new_transport.0);

            std::mem::drop(new_transport);

            // Set listeners again for an update Rc in a listener.
            //
            // If we don't do this then in a listener we will have
            // pointer to old WebSocketRpcTransport.
            this.set_on_close_listener()?;
            this.set_on_message_listener()?;

            this.0.borrow_mut().sync_socket_state();

            Ok(())
        })
    }

    fn get_state(&self) -> State {
        self.0.borrow().socket_state
    }

    fn on_state_change(&self) -> LocalBoxStream<'static, State> {
        let (tx, rx) = mpsc::unbounded();
        self.0.borrow_mut().on_state_change_subs.push(tx);

        Box::pin(rx)
    }
}

/// [`Weak`] pointer which can be upgraded to [`WebSocketRpcTransport`].
pub struct WeakWebSocketRpcTransport(Weak<RefCell<InnerSocket>>);

impl WeakWebSocketRpcTransport {
    /// Returns [`WeakWebSocketRpcTransport`] with [`Weak`] pointer to a
    /// provided [`WebSocketRpcTransport`].
    pub fn new(strong: &WebSocketRpcTransport) -> Self {
        Self(Rc::downgrade(&strong.0))
    }

    /// Returns `Some(WebSocketRpcTransport)` if it still exists.
    pub fn upgrade(&self) -> Option<WebSocketRpcTransport> {
        self.0.upgrade().map(WebSocketRpcTransport)
    }
}

impl WebSocketRpcTransport {
    /// Initiates new WebSocket connection. Resolves only when underlying
    /// connection becomes active.
    pub async fn new(url: String) -> Result<Self> {
        let (tx_close, rx_close) = oneshot::channel();
        let (tx_open, rx_open) = oneshot::channel();

        let inner = InnerSocket::new(url)?;
        let socket = Self(Rc::new(RefCell::new(inner)));

        {
            let inner = socket.downgrade();
            let socket_transport = socket.0.borrow().socket.clone();
            socket.0.borrow_mut().on_close_listener = Some(
                EventListener::new_once(
                    Rc::clone(&socket_transport),
                    "close",
                    move |_| {
                        if let Some(inner) = inner.upgrade() {
                            inner.0.borrow_mut().sync_socket_state();
                        }
                        let _ = tx_close.send(());
                    },
                )
                .map_err(tracerr::map_from_and_wrap!())?,
            );

            let inner = socket.downgrade();
            socket.0.borrow_mut().on_open_listener = Some(
                EventListener::new_once(
                    Rc::clone(&socket_transport),
                    "open",
                    move |_| {
                        if let Some(inner) = inner.upgrade() {
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
        let weak_transport = self.downgrade();
        let on_close = EventListener::new_once(
            Rc::clone(&self.0.borrow().socket),
            "close",
            move |msg: CloseEvent| {
                let close_msg = CloseMsg::from(&msg);
                let transport =
                    if let Some(socket_clone) = weak_transport.upgrade() {
                        socket_clone
                    } else {
                        console_error(
                            "'WebSocketRpcTransport' was unexpectedly gone.",
                        );
                        return;
                    };
                let mut transport_ref_mut = transport.0.borrow_mut();
                transport_ref_mut.sync_socket_state();
                transport_ref_mut
                    .on_close_subs
                    .retain(|on_close| !on_close.is_closed());

                transport_ref_mut.on_close_subs.iter().for_each(|on_close| {
                    on_close.unbounded_send(close_msg.clone()).unwrap_or_else(
                        |e| {
                            console_error(format!(
                                "WebSocket's 'on_close' callback receiver \
                                 unexpectedly gone. {:?}",
                                e
                            ))
                        },
                    );
                })
            },
        )
        .map_err(tracerr::map_from_and_wrap!(=> TransportError))?;
        self.0.borrow_mut().on_close_listener = Some(on_close);

        Ok(())
    }

    /// Sets [`WebSocketRpcTransport::on_message_listener`] which will send
    /// [`ServerMessage`]s to [`WebSocketRpcTransport::on_message`].
    fn set_on_message_listener(&self) -> Result<()> {
        let weak_transport = self.downgrade();
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
                let transport =
                    if let Some(transport) = weak_transport.upgrade() {
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

    /// Downgrades strong ([`Rc`]) pointed [`WebSocketRpcTransport`] to a
    /// [`Weak`] pointed [`WeakWebSocketRpcTransport`].
    fn downgrade(&self) -> WeakWebSocketRpcTransport {
        WeakWebSocketRpcTransport::new(self)
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

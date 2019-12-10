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
    utils::{EventListener, EventListenerBindError, JsCaused, JsError},
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
#[derive(From, Into)]
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
#[derive(Debug)]
enum State {
    Connecting,
    Open,
    Closing,
    Closed,
}

impl State {
    /// Returns `true` if socket can be closed.
    pub fn can_close(&self) -> bool {
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

    /// Listener for [`WebSocket`] [`error`] event.
    ///
    /// [`WebSocket`]:
    /// https://developer.mozilla.org/en-US/docs/Web/API/WebSocket
    /// [`error`]:
    /// https://developer.mozilla.org/en-US/docs/Web/API/WebSocket/error_event
    on_error_listener: Option<EventListener<SysWebSocket, Event>>,

    /// [`mpsc::UnboundedSender`] for [`RpcTransport::on_message`]'s
    /// [`LocalBoxStream`].
    on_message: Option<mpsc::UnboundedSender<Result<ServerMsg>>>,

    /// [`mpsc::UnboundedSender`] for [`RpcTransport::on_close`]'s
    /// [`LocalBoxStream`].
    on_close: Option<mpsc::UnboundedSender<CloseMsg>>,

    /// Reason of [`WebSocketRpcTransport`] closing. Will be sent in
    /// `WebSocket` [close frame].
    ///
    /// [close frame]:
    /// https://tools.ietf.org/html/rfc6455#section-5.5.1
    close_reason: ClientDisconnect,

    /// URL to which this [`WebSocketRpcTransport`] is connected.
    url: String,
}

/// WebSocket [`RpcTransport`] between client and server.
#[derive(Clone)]
pub struct WebSocketRpcTransport(Rc<RefCell<InnerSocket>>);

impl InnerSocket {
    fn new(url: &str) -> Result<Self> {
        let socket = SysWebSocket::new(url)
            .map_err(Into::into)
            .map_err(TransportError::CreateSocket)
            .map_err(tracerr::wrap!())?;
        Ok(Self {
            socket_state: State::Connecting,
            socket: Rc::new(socket),
            on_open_listener: None,
            on_message_listener: None,
            on_close_listener: None,
            on_error_listener: None,
            on_close: None,
            on_message: None,
            close_reason: ClientDisconnect::RpcTransportUnexpectedlyDropped,
            url: url.to_string(),
        })
    }

    /// Checks underlying WebSocket state and updates `socket_state`.
    fn update_state(&mut self) {
        self.socket_state = self.socket.ready_state().into();
    }
}

impl RpcTransport for WebSocketRpcTransport {
    fn on_message(&self) -> Result<LocalBoxStream<'static, Result<ServerMsg>>> {
        let (tx, rx) = mpsc::unbounded();
        self.0.borrow_mut().on_message = Some(tx);

        Ok(Box::pin(rx))
    }

    fn on_close(&self) -> Result<LocalBoxStream<'static, CloseMsg>> {
        let (tx, rx) = mpsc::unbounded();
        self.0.borrow_mut().on_close = Some(tx);

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
        let this = self.clone();
        Box::pin(async move {
            let url = this.0.borrow().url.clone();
            let new_transport = Self::new(&url).await?;
            new_transport.0.borrow_mut().on_message =
                this.0.borrow_mut().on_message.take();
            new_transport.0.borrow_mut().on_close =
                this.0.borrow_mut().on_close.take();

            RefCell::swap(&this.0, &new_transport.0);
            let old_transport = new_transport;

            // Take all listeners because we have pointers in them (cyclic
            // dependency).
            old_transport.0.borrow_mut().on_open_listener.take();
            old_transport.0.borrow_mut().on_error_listener.take();
            old_transport.0.borrow_mut().on_message_listener.take();
            old_transport.0.borrow_mut().on_close_listener.take();

            // Set listeners again for an update Rc in a listener.
            //
            // If we don't do this then in a listener we will have
            // pointer to old WebSocketRpcTransport.
            this.set_on_close_listener()?;
            this.set_on_message_listener()?;

            this.0.borrow_mut().update_state();

            Ok(())
        })
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
    pub async fn new(url: &str) -> Result<Self> {
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
                            inner.0.borrow_mut().update_state();
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
                            inner.0.borrow_mut().update_state();
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
        let socket_clone = self.clone();
        let on_close = EventListener::new_once(
            Rc::clone(&self.0.borrow().socket),
            "close",
            move |msg: CloseEvent| {
                let close_msg = CloseMsg::from(&msg);
                socket_clone.0.borrow_mut().update_state();
                if let Some(on_close) =
                    socket_clone.0.borrow().on_close.as_ref()
                {
                    on_close.unbounded_send(close_msg).unwrap_or_else(|e| {
                        console_error!(format!(
                            "WebSocket's 'on_close' callback receiver \
                             unexpectedly gone. {:?}",
                            e
                        ))
                    });
                }
            },
        )
        .map_err(tracerr::map_from_and_wrap!(=> TransportError))?;
        self.0.borrow_mut().on_close_listener = Some(on_close);

        Ok(())
    }

    /// Sets [`WebSocketRpcTransport::on_message_listener`] which will send
    /// [`ServerMessage`]s to [`WebSocketRpcTransport::on_message`].
    fn set_on_message_listener(&self) -> Result<()> {
        let socket_clone = self.clone();
        let on_message = EventListener::new_mut(
            Rc::clone(&self.0.borrow().socket),
            "message",
            move |msg| {
                let parsed = ServerMessage::try_from(&msg)
                    .map(Into::into)
                    .map_err(tracerr::wrap!());
                if let Some(on_message) =
                    socket_clone.0.borrow().on_message.as_ref()
                {
                    on_message.unbounded_send(parsed).unwrap_or_else(|e| {
                        console_error!(format!(
                            "WebSocket's 'on_message' callback receiver \
                             unexpectedly gone. {:?}",
                            e
                        ))
                    });
                }
            },
        )
        .map_err(tracerr::map_from_and_wrap!(=> TransportError))?;

        self.0.borrow_mut().on_message_listener = Some(on_message);

        Ok(())
    }

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
                console_error!(err);
            }
        }
    }
}

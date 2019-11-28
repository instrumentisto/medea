//! [WebSocket] transport wrapper.
//!
//! [WebSocket]: https://developer.mozilla.org/ru/docs/WebSockets

use std::{cell::RefCell, convert::TryFrom, rc::Rc};

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
    rpc::{CloseMsg, RpcTransport},
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

    /// Occurs when message is sent to closed socket.
    #[display(fmt = "Underlying socket is closed")]
    ClosedSocket,
}

impl From<EventListenerBindError> for TransportError {
    fn from(err: EventListenerBindError) -> Self {
        Self::WebSocketEventBindError(err)
    }
}

type Result<T, E = Traced<TransportError>> = std::result::Result<T, E>;

/// State of websocket.
#[derive(Debug)]
enum State {
    CONNECTING,
    OPEN,
    CLOSING,
    CLOSED,
}

impl State {
    /// Returns `true` if socket can be closed.
    pub fn can_close(&self) -> bool {
        match self {
            Self::CONNECTING | Self::OPEN => true,
            _ => false,
        }
    }
}

impl From<u16> for State {
    fn from(value: u16) -> Self {
        match value {
            0 => Self::CONNECTING,
            1 => Self::OPEN,
            2 => Self::CLOSING,
            3 => Self::CLOSED,
            _ => unreachable!(),
        }
    }
}

struct InnerSocket {
    socket: Rc<SysWebSocket>,
    socket_state: State,
    on_open: Option<EventListener<SysWebSocket, Event>>,
    on_message: Option<EventListener<SysWebSocket, MessageEvent>>,
    on_close: Option<EventListener<SysWebSocket, CloseEvent>>,
    on_error: Option<EventListener<SysWebSocket, Event>>,
}

/// WebSocket [`RpcTransport`] between client and server.
pub struct WebSocketRpcTransport(Rc<RefCell<InnerSocket>>);

impl InnerSocket {
    fn new(url: &str) -> Result<Self> {
        let socket = SysWebSocket::new(url)
            .map_err(Into::into)
            .map_err(TransportError::CreateSocket)
            .map_err(tracerr::wrap!())?;
        Ok(Self {
            socket_state: State::CONNECTING,
            socket: Rc::new(socket),
            on_open: None,
            on_message: None,
            on_close: None,
            on_error: None,
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
        let mut inner_mut = self.0.borrow_mut();
        inner_mut.on_message = Some(
            EventListener::new_mut(
                Rc::clone(&inner_mut.socket),
                "message",
                move |msg| {
                    let parsed = ServerMessage::try_from(&msg)
                        .map(Into::into)
                        .map_err(tracerr::wrap!());
                    tx.unbounded_send(parsed).unwrap_or_else(|e| {
                        console_error!(format!(
                            "WebSocket's 'on_message' callback receiver \
                             unexpectedly gone. {:?}",
                            e
                        ))
                    });
                },
            )
            .map_err(tracerr::map_from_and_wrap!(=> TransportError))?,
        );
        Ok(Box::pin(rx))
    }

    fn on_close(
        &self,
    ) -> Result<LocalBoxFuture<'static, Result<CloseMsg, oneshot::Canceled>>>
    {
        let (tx, rx) = oneshot::channel();
        let mut inner_mut = self.0.borrow_mut();
        let inner = Rc::clone(&self.0);
        inner_mut.on_close = Some(
            EventListener::new_once(
                Rc::clone(&inner_mut.socket),
                "close",
                move |msg: CloseEvent| {
                    inner.borrow_mut().update_state();
                    tx.send(CloseMsg::from(&msg)).unwrap_or_else(|e| {
                        console_error!(format!(
                            "WebSocket's 'on_close' callback receiver \
                             unexpectedly gone. {:?}",
                            e
                        ))
                    });
                },
            )
            .map_err(tracerr::map_from_and_wrap!(=> TransportError))?,
        );
        Ok(Box::pin(rx))
    }

    fn send(&self, msg: &ClientMsg) -> Result<()> {
        let inner = self.0.borrow();
        let message = serde_json::to_string(msg)
            .map_err(TransportError::ParseClientMessage)
            .map_err(tracerr::wrap!())?;

        match inner.socket_state {
            State::OPEN => inner
                .socket
                .send_with_str(&message)
                .map_err(Into::into)
                .map_err(TransportError::SendMessage)
                .map_err(tracerr::wrap!()),
            _ => Err(tracerr::new!(TransportError::ClosedSocket)),
        }
    }
}

impl WebSocketRpcTransport {
    /// Initiates new WebSocket connection. Resolves only when underlying
    /// connection becomes active.
    pub async fn new(url: &str) -> Result<Self> {
        let (tx_close, rx_close) = oneshot::channel();
        let (tx_open, rx_open) = oneshot::channel();

        let inner = InnerSocket::new(url)?;
        let socket = Rc::new(RefCell::new(inner));

        {
            let mut socket_mut = socket.borrow_mut();
            let inner = Rc::clone(&socket);
            socket_mut.on_close = Some(
                EventListener::new_once(
                    Rc::clone(&socket_mut.socket),
                    "close",
                    move |_| {
                        inner.borrow_mut().update_state();
                        let _ = tx_close.send(());
                    },
                )
                .map_err(tracerr::map_from_and_wrap!())?,
            );

            let inner = Rc::clone(&socket);
            socket_mut.on_open = Some(
                EventListener::new_once(
                    Rc::clone(&socket_mut.socket),
                    "open",
                    move |_| {
                        inner.borrow_mut().update_state();
                        let _ = tx_open.send(());
                    },
                )
                .map_err(tracerr::map_from_and_wrap!(=> TransportError))?,
            );
        }

        let state = future::select(rx_open, rx_close).await;

        socket.borrow_mut().on_open.take();
        socket.borrow_mut().on_close.take();

        match state {
            future::Either::Left((opened, _)) => match opened {
                Ok(_) => Ok(Self(socket)),
                Err(_) => Err(tracerr::new!(TransportError::InitSocket)),
            },
            future::Either::Right(_closed) => {
                Err(tracerr::new!(TransportError::InitSocket))
            }
        }
    }
}

impl Drop for WebSocketRpcTransport {
    fn drop(&mut self) {
        let mut inner = self.0.borrow_mut();
        if inner.socket_state.can_close() {
            inner.on_open.take();
            inner.on_error.take();
            inner.on_message.take();
            inner.on_close.take();

            if let Err(err) = inner
                .socket
                .close_with_code_and_reason(1000, "Dropped unexpectedly")
            {
                console_error!(err);
            }
        }
    }
}

impl From<&CloseEvent> for CloseMsg {
    fn from(event: &CloseEvent) -> Self {
        let code: u16 = event.code();
        let body = format!("{}:{}", code, event.reason());
        match code {
            1000 => Self::Normal(body),
            _ => Self::Disconnect(body),
        }
    }
}

/// Wrapper for help to get [`ServerMsg`] from Websocket [`MessageEvent`][1].
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

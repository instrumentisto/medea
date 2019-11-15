//! [WebSocket] transport wrapper.
//!
//! [WebSocket]: https://developer.mozilla.org/ru/docs/WebSockets

use std::{cell::RefCell, convert::TryFrom, rc::Rc};

use derive_more::*;
use futures::{channel::oneshot, future};
use medea_client_api_proto::{ClientMsg, ServerMsg};
use tracerr::Traced;
use web_sys::{CloseEvent, Event, MessageEvent, WebSocket as SysWebSocket};

use crate::{
    rpc::CloseMsg,
    utils::{EventListener, JasonError, JsCaused, JsError},
};

/// Errors that may occur when working with [`WebSocket`].
#[derive(Debug, Display, JsCaused)]
pub enum SocketError {
    /// Occurs when the port to which the connection is being attempted
    /// is being blocked.
    #[display(fmt = "failed to create WebSocket: {}", _0)]
    CreateSocket(JsError),

    /// Occurs when the connection close before becomes state active.
    #[display(fmt = "failed to init WebSocket")]
    InitSocket,

    /// Occurs when [`ClientMessage`] cannot be parsed.
    #[display(fmt = "failed to parse client message: {}", _0)]
    ParseClientMessage(serde_json::error::Error),

    /// Occurs when [`ServerMessage`] cannot be parsed.
    #[display(fmt = "failed to parse server message: {}", _0)]
    ParseServerMessage(serde_json::error::Error),

    /// Occurs if the parsed message is not string.
    #[display(fmt = "message is not a string")]
    MessageNotString,

    /// Occurs when a message cannot be send to server.
    #[display(fmt = "failed to send message: {}", _0)]
    SendMessage(JsError),

    /// Occurs when handler cannot be set for the event [`CloseEvent`].
    #[display(fmt = "failed to set handler for CloseEvent: {}", _0)]
    SetHandlerOnClose(JsError),

    /// Occurs when handler cannot be set for the event [`OpenEvent`].
    #[display(fmt = "failed to set handler for OpenEvent: {}", _0)]
    SetHandlerOnOpen(JsError),

    /// Occurs when handler cannot be set for the event [`MessageEvent`].
    #[display(fmt = "failed to set handler for MessageEvent: {}", _0)]
    SetHandlerOnMessage(JsError),

    /// Occurs when underlying WebSocket state cannot be checked.
    #[display(fmt = "could not cast {} to State variant", _0)]
    CastState(u16),

    /// Occurs when message is sent to closed socket.
    #[display(fmt = "underlying socket is closed")]
    ClosedSocket,
}

type Result<T> = std::result::Result<T, Traced<SocketError>>;

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

impl TryFrom<u16> for State {
    type Error = SocketError;

    fn try_from(value: u16) -> std::result::Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::CONNECTING),
            1 => Ok(Self::OPEN),
            2 => Ok(Self::CLOSING),
            3 => Ok(Self::CLOSED),
            _ => Err(SocketError::CastState(value)),
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

pub struct WebSocket(Rc<RefCell<InnerSocket>>);

impl InnerSocket {
    fn new(url: &str) -> Result<Self> {
        let socket = SysWebSocket::new(url)
            .map_err(Into::into)
            .map_err(SocketError::CreateSocket)
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
        match State::try_from(self.socket.ready_state()) {
            Ok(new_state) => self.socket_state = new_state,
            Err(err) => {
                // unreachable, unless some vendor will break enum
                console_error!(JasonError::from(
                    tracerr::new!(err).into_parts()
                )
                .to_string())
            }
        };
    }
}

impl WebSocket {
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
                .map_err(SocketError::SetHandlerOnClose)
                .map_err(tracerr::wrap!())?,
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
                .map_err(SocketError::SetHandlerOnOpen)
                .map_err(tracerr::wrap!())?,
            );
        }

        let state = future::select(rx_open, rx_close).await;

        socket.borrow_mut().on_open.take();
        socket.borrow_mut().on_close.take();

        match state {
            future::Either::Left((opened, _)) => match opened {
                Ok(_) => Ok(Self(socket)),
                Err(_) => Err(tracerr::new!(SocketError::InitSocket)),
            },
            future::Either::Right(_closed) => {
                Err(tracerr::new!(SocketError::InitSocket))
            }
        }
    }

    /// Set handler on receive message from server.
    pub fn on_message<F>(&self, mut f: F) -> Result<()>
    where
        F: (FnMut(Result<ServerMsg>)) + 'static,
    {
        let mut inner_mut = self.0.borrow_mut();
        inner_mut.on_message = Some(
            EventListener::new_mut(
                Rc::clone(&inner_mut.socket),
                "message",
                move |msg| {
                    let parsed = ServerMessage::try_from(&msg)
                        .map(std::convert::Into::into)
                        .map_err(tracerr::wrap!());
                    f(parsed);
                },
            )
            .map_err(Into::into)
            .map_err(SocketError::SetHandlerOnMessage)
            .map_err(tracerr::wrap!())?,
        );
        Ok(())
    }

    /// Set handler on close socket.
    pub fn on_close<F>(&self, f: F) -> Result<()>
    where
        F: (FnOnce(CloseMsg)) + 'static,
    {
        let mut inner_mut = self.0.borrow_mut();
        let inner = Rc::clone(&self.0);
        inner_mut.on_close = Some(
            EventListener::new_once(
                Rc::clone(&inner_mut.socket),
                "close",
                move |msg: CloseEvent| {
                    inner.borrow_mut().update_state();
                    f(CloseMsg::from(&msg));
                },
            )
            .map_err(SocketError::SetHandlerOnClose)
            .map_err(tracerr::wrap!())?,
        );
        Ok(())
    }

    /// Send message to server.
    pub fn send(&self, msg: &ClientMsg) -> Result<()> {
        let inner = self.0.borrow();
        let message = serde_json::to_string(msg)
            .map_err(SocketError::ParseClientMessage)
            .map_err(tracerr::wrap!())?;

        match inner.socket_state {
            State::OPEN => inner
                .socket
                .send_with_str(&message)
                .map_err(Into::into)
                .map_err(SocketError::SendMessage)
                .map_err(tracerr::wrap!()),
            _ => Err(tracerr::new!(SocketError::ClosedSocket)),
        }
    }
}

impl Drop for WebSocket {
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
    type Error = SocketError;

    fn try_from(msg: &MessageEvent) -> std::result::Result<Self, Self::Error> {
        use SocketError::*;
        let payload = msg.data().as_string().ok_or(MessageNotString)?;

        serde_json::from_str::<ServerMsg>(&payload)
            .map_err(ParseServerMessage)
            .map(Self::from)
    }
}

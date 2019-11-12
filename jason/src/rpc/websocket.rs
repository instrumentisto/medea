//! [WebSocket] transport wrapper.
//!
//! [WebSocket]: https://developer.mozilla.org/ru/docs/WebSockets

use std::{cell::RefCell, convert::TryFrom, rc::Rc};

use derive_more::Display;
use futures::{channel::oneshot, future};
use macro_attr::*;
use medea_client_api_proto::{ClientMsg, ServerMsg};
use newtype_derive::NewtypeFrom;
use tracerr::Traced;
use web_sys::{CloseEvent, Event, MessageEvent, WebSocket as SysWebSocket};

use crate::{
    rpc::CloseMsg,
    utils::{EventListener, JasonError, JsCaused, JsError},
};

/// Errors that may occur when working with [`WebSocket`].
#[derive(Debug, Display)]
pub enum SocketError {
    #[display(fmt = "failed to create WebSocket: {}", _0)]
    CreateSocket(JsError),
    #[display(fmt = "failed to init WebSocket")]
    InitSocket,
    #[display(fmt = "failed to parse client message: {}", _0)]
    ParseClientMessage(serde_json::error::Error),
    #[display(fmt = "failed to parse server message: {}", _0)]
    ParseServerMessage(serde_json::error::Error),
    #[display(fmt = "message is not a string")]
    MessageNotString,
    #[display(fmt = "failed to send message: {}", _0)]
    SendMessage(JsError),
    #[display(fmt = "failed to set handler for CloseEvent: {}", _0)]
    SetHandlerOnClose(JsError),
    #[display(fmt = "failed to set handler for OpenEvent: {}", _0)]
    SetHandlerOnOpen(JsError),
    #[display(fmt = "failed to set handler for MessageEvent: {}", _0)]
    SetHandlerOnMessage(JsError),
    #[display(fmt = "could not cast {} to State variant", _0)]
    CastState(u16),
    #[display(fmt = "underlying socket is closed")]
    ClosedSocket,
}

impl JsCaused for SocketError {
    fn name(&self) -> &'static str {
        use SocketError::*;
        match self {
            CreateSocket(_) => "CreateSocket",
            InitSocket => "InitSocket",
            ParseClientMessage(_) => "ParseClientMessage",
            ParseServerMessage(_) => "ParseServerMessage",
            MessageNotString => "MessageNotString",
            SendMessage(_) => "SendMessage",
            SetHandlerOnClose(_) => "SetHandlerOnClose",
            SetHandlerOnOpen(_) => "SetHandlerOnOpen",
            SetHandlerOnMessage(_) => "SetHandlerOnMessage",
            CastState(_) => "CastState",
            ClosedSocket => "ClosedSocket",
        }
    }

    fn js_cause(&self) -> Option<js_sys::Error> {
        use SocketError::*;
        match self {
            InitSocket
            | ClosedSocket
            | MessageNotString
            | ParseClientMessage(_)
            | ParseServerMessage(_)
            | CastState(_) => None,
            CreateSocket(err)
            | SendMessage(err)
            | SetHandlerOnClose(err)
            | SetHandlerOnOpen(err)
            | SetHandlerOnMessage(err) => err.js_cause(),
        }
    }
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
                console_error!(
                    JasonError::from(tracerr::new!(err).unwrap()).to_string()
                )
            }
        };
    }
}

impl WebSocket {
    /// Initiates new WebSocket connection. Resolves only when underlying
    /// connection becomes active.
    pub async fn new(url: &str) -> Result<Self> {
        use SocketError::*;
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
                .map_err(SetHandlerOnClose)
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
                .map_err(SetHandlerOnOpen)
                .map_err(tracerr::wrap!())?,
            );
        }

        let state = future::select(rx_open, rx_close).await;

        socket.borrow_mut().on_open.take();
        socket.borrow_mut().on_close.take();

        match state {
            future::Either::Left((opened, _)) => match opened {
                Ok(_) => Ok(Self(socket)),
                Err(_) => Err(tracerr::new!(InitSocket)),
            },
            future::Either::Right(_closed) => Err(tracerr::new!(InitSocket)),
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
        use SocketError::*;
        let inner = self.0.borrow();
        let message = serde_json::to_string(msg)
            .map_err(ParseClientMessage)
            .map_err(tracerr::wrap!())?;

        match inner.socket_state {
            State::OPEN => inner
                .socket
                .send_with_str(&message)
                .map_err(Into::into)
                .map_err(SendMessage)
                .map_err(tracerr::wrap!()),
            _ => Err(tracerr::new!(ClosedSocket)),
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

// TODO use derive_more::From
macro_attr! {
    #[derive(NewtypeFrom!)]
    pub struct ServerMessage(ServerMsg);
}

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

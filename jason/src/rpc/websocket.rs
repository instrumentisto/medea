//! [WebSocket] transport wrapper.
//!
//! [WebSocket]: https://developer.mozilla.org/ru/docs/WebSockets

use std::{cell::RefCell, convert::TryFrom, rc::Rc};

use derive_more::{From, Into};
use futures::{
    channel::{mpsc, oneshot},
    future,
};
use medea_client_api_proto::{ClientMsg, ServerMsg};
use thiserror::*;
use web_sys::{CloseEvent, Event, MessageEvent, WebSocket as SysWebSocket};

use crate::{
    rpc::{CloseMsg, RpcTransport},
    utils::{EventListener, WasmErr},
};

/// Errors that may occur when working with [`WebSocket`].
#[derive(Debug, Error)]
pub enum Error {
    /// Failed to create WebSocket.
    #[error("failed to create WebSocket: {0}")]
    CreateSocket(WasmErr),

    /// Failed to init WebSocket.
    #[error("failed to init WebSocket")]
    InitSocket,

    /// Failed to parse client message.
    #[error("failed to parse client message: {0}")]
    ParseClientMessage(serde_json::error::Error),

    /// Failed to parse server message.
    #[error("failed to parse server message: {0}")]
    ParseServerMessage(serde_json::error::Error),

    /// Message is not string.
    #[error("message is not a string")]
    MessageNotString,

    /// Failed to send message.
    #[error("failed to send message: {0}")]
    SendMessage(WasmErr),

    /// Failed to set handler for CloseEvent.
    #[error("failed to set handler for CloseEvent: {0}")]
    SetHandlerOnClose(WasmErr),

    /// Failed to set handler for OpenEvent.
    #[error("failed to set handler for OpenEvent: {0}")]
    SetHandlerOnOpen(WasmErr),

    /// Failed to set handler for MessageEvent.
    #[error("failed to set handler for MessageEvent: {0}")]
    SetHandlerOnMessage(WasmErr),

    /// Couldn't cast provided [`u16`] to `State` variant.
    #[error("could not cast {0} to State variant")]
    CastState(u16),

    /// Underlying socket is closed.
    #[error("underlying socket is closed")]
    ClosedSocket,
}

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
    type Error = Error;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::CONNECTING),
            1 => Ok(Self::OPEN),
            2 => Ok(Self::CLOSING),
            3 => Ok(Self::CLOSED),
            _ => Err(Error::CastState(value)),
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
pub struct WebSocket(Rc<RefCell<InnerSocket>>);

impl InnerSocket {
    fn new(url: &str) -> Result<Self, Error> {
        let socket = SysWebSocket::new(url)
            .map_err(Into::into)
            .map_err(Error::CreateSocket)?;
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
                console_error!(err.to_string())
            }
        };
    }
}

impl RpcTransport for WebSocket {
    /// Sets handler for receiving server messages.
    fn on_message(
        &self,
    ) -> Result<mpsc::UnboundedReceiver<Result<ServerMsg, Error>>, Error> {
        let (tx, rx) = mpsc::unbounded();
        let mut inner_mut = self.0.borrow_mut();
        inner_mut.on_message = Some(
            EventListener::new_mut(
                Rc::clone(&inner_mut.socket),
                "message",
                move |msg| {
                    let parsed = ServerMessage::try_from(&msg).map(Into::into);
                    tx.unbounded_send(parsed).unwrap_or_else(|e| {
                        console_error!(format!(
                            "WebSocket's 'on_message' callback receiver \
                             unexpectedly gone. {:?}",
                            e
                        ))
                    });
                },
            )
            .map_err(Into::into)
            .map_err(Error::SetHandlerOnMessage)?,
        );
        Ok(rx)
    }

    /// Sets handler for socket closing.
    fn on_close(&self) -> Result<oneshot::Receiver<CloseMsg>, Error> {
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
            .map_err(Error::SetHandlerOnClose)?,
        );
        Ok(rx)
    }

    /// Sends message to the server.
    fn send(&self, msg: &ClientMsg) -> Result<(), Error> {
        let inner = self.0.borrow();
        let message =
            serde_json::to_string(msg).map_err(Error::ParseClientMessage)?;

        match inner.socket_state {
            State::OPEN => inner
                .socket
                .send_with_str(&message)
                .map_err(std::convert::Into::into)
                .map_err(Error::SendMessage),
            _ => Err(Error::ClosedSocket),
        }
    }
}

impl WebSocket {
    /// Initiates new WebSocket connection. Resolves only when underlying
    /// connection becomes active.
    pub async fn new(url: &str) -> Result<Self, Error> {
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
                .map_err(Error::SetHandlerOnClose)?,
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
                .map_err(Error::SetHandlerOnOpen)?,
            );
        }

        let state = future::select(rx_open, rx_close).await;

        socket.borrow_mut().on_open.take();
        socket.borrow_mut().on_close.take();

        match state {
            future::Either::Left((opened, _)) => match opened {
                Ok(_) => Ok(Self(socket)),
                Err(_) => Err(Error::InitSocket),
            },
            future::Either::Right(_closed) => Err(Error::InitSocket),
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

/// Newtype for received server message.
#[derive(From, Into)]
pub struct ServerMessage(ServerMsg);

impl TryFrom<&MessageEvent> for ServerMessage {
    type Error = Error;

    fn try_from(msg: &MessageEvent) -> std::result::Result<Self, Self::Error> {
        let payload = msg.data().as_string().ok_or(Error::MessageNotString)?;

        serde_json::from_str::<ServerMsg>(&payload)
            .map_err(Error::ParseServerMessage)
            .map(Self::from)
    }
}

//! [WebSocket] transport wrapper.
//!
//! [WebSocket]: https://developer.mozilla.org/ru/docs/WebSockets

use std::{borrow::Cow, cell::RefCell, convert::TryFrom, rc::Rc};

use derive_more::{Display, From, Into};
use futures::{
    channel::{mpsc, oneshot},
    future::{self, LocalBoxFuture},
    stream::LocalBoxStream,
    SinkExt,
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
    on_open_listener: Option<EventListener<SysWebSocket, Event>>,
    on_message_listener: Option<EventListener<SysWebSocket, MessageEvent>>,
    on_close_listener: Option<EventListener<SysWebSocket, CloseEvent>>,
    on_error_listener: Option<EventListener<SysWebSocket, Event>>,
    on_open: Option<mpsc::UnboundedSender<Event>>,
    on_message: Option<mpsc::UnboundedSender<Result<ServerMsg>>>,
    on_close: Option<mpsc::UnboundedSender<CloseMsg>>,
    on_error: Option<mpsc::UnboundedSender<Event>>,
    close_reason: ClientDisconnect,
    token: String,
}

/// WebSocket [`RpcTransport`] between client and server.
pub struct WebSocketRpcTransport(Rc<RefCell<InnerSocket>>);

impl Clone for WebSocketRpcTransport {
    fn clone(&self) -> Self {
        console_error!(format!("{:?}", backtrace::Backtrace::new()));
        Self(Rc::clone(&self.0))
    }
}

impl InnerSocket {
    fn new(url: &str) -> Result<Self> {
        let socket = SysWebSocket::new(url)
            .map_err(Into::into)
            .map_err(TransportError::CreateSocket)
            .map_err(tracerr::wrap!())?;
        Ok(Self {
            socket_state: State::CONNECTING,
            socket: Rc::new(socket),
            on_open_listener: None,
            on_message_listener: None,
            on_close_listener: None,
            on_error_listener: None,
            on_open: None,
            on_error: None,
            on_close: None,
            on_message: None,
            close_reason: ClientDisconnect::RpcTransportUnexpectedlyDropped,
            token: url.to_string(),
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
            State::OPEN => inner
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

    // Parent have Rc
    fn reconnect(&self) -> LocalBoxFuture<'static, Result<()>> {
        console_error!("Trying to reconnect WebSocket...");
        // Rc clone
        let self_clone = self.clone();
        let on_message = self.0.borrow_mut().on_message.take();
        let on_close = self.0.borrow_mut().on_close.take();
        Box::pin(async move {
            let url = self_clone.0.borrow().token.clone();
            console_error!("Creating new socket...");
            let mut new_transport = Self::new(&url).await.unwrap();
            console_error!("Socket successfully created!");
            new_transport.0.borrow_mut().on_message = on_message;
            new_transport.0.borrow_mut().on_close = on_close;
            console_error!(format!("{} -- {}", line!(), Rc::strong_count(&self_clone.0)));
            RefCell::swap(&self_clone.0, &new_transport.0);
            let old_transport = new_transport;
            self_clone.set_on_close_listener()?;
            self_clone.set_on_message_listener()?;
            old_transport.0.borrow_mut().on_open_listener.take();
            old_transport.0.borrow_mut().on_error_listener.take();
            old_transport.0.borrow_mut().on_message_listener.take();
            old_transport.0.borrow_mut().on_close_listener.take();
            console_error!(format!("{} -- {}", line!(), Rc::strong_count(&old_transport.0)));
            console_error!(format!(
                "on_close: {:?}",
                self_clone.0.borrow().on_close
            ));
            console_error!("Swapped old socket with new!");
            self_clone.0.borrow_mut().update_state();
            Ok(())
        })
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
        console_error!(format!("{} -- {}", line!(), Rc::strong_count(&socket.0)));

        {
            let mut socket_mut = socket.0.borrow_mut();
            let inner = socket.clone();
            socket_mut.on_close_listener = Some(
                EventListener::new_once(
                    Rc::clone(&socket_mut.socket),
                    "close",
                    move |_| {
                        inner.0.borrow_mut().update_state();
                        let _ = tx_close.send(());
                    },
                )
                .map_err(tracerr::map_from_and_wrap!())?,
            );
            console_error!(format!("{} -- {}", line!(), Rc::strong_count(&socket.0)));

            let inner = socket.clone();
            socket_mut.on_open_listener = Some(
                EventListener::new_once(
                    Rc::clone(&socket_mut.socket),
                    "open",
                    move |_| {
                        inner.0.borrow_mut().update_state();
                        let _ = tx_open.send(());
                    },
                )
                .map_err(tracerr::map_from_and_wrap!(=> TransportError))?,
            );
            console_error!(format!("{} -- {}", line!(), Rc::strong_count(&socket.0)));
        }

        let state = future::select(rx_open, rx_close).await;

        console_error!(format!("{} -- {}", line!(), Rc::strong_count(&socket.0)));
        socket.0.borrow_mut().on_open_listener.take();
        console_error!(format!("{} -- {}", line!(), Rc::strong_count(&socket.0)));
        socket.0.borrow_mut().on_close_listener.take();
        console_error!(format!("{} -- {}", line!(), Rc::strong_count(&socket.0)));

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

    fn set_on_close_listener(&self) -> Result<()> {
        let socket_clone = self.clone();
        let on_close = EventListener::new_once(
            Rc::clone(&self.0.borrow().socket),
            "close",
            move |msg: CloseEvent| {
                let close_msg = CloseMsg::from(&msg);
                console_error!(format!("OnClose {:?}", close_msg));
                console_error!(format!(
                    "OnMessage {:?}",
                    socket_clone.0.borrow().on_message
                ));
                socket_clone.0.borrow_mut().update_state();
                if let Some(on_close) = socket_clone.0.borrow().on_close.as_ref()
                {
                    on_close.unbounded_send(close_msg).unwrap_or_else(|e| {
                        console_error!(format!(
                            "WebSocket's 'on_close' callback receiver \
                             unexpectedly gone. {:?}",
                            e
                        ))
                    });
                } else {
                    console_error!("No future for on_close");
                }
            },
        )
            .map_err(tracerr::map_from_and_wrap!(=> TransportError))?;
        self.0.borrow_mut().on_close_listener = Some(on_close);
        console_error!(format!("{} -- {}", line!(), Rc::strong_count(&self.0)));

        Ok(())
    }

    fn set_on_message_listener(&self) -> Result<()> {
        let socket_clone = self.clone();
        let on_message = EventListener::new_mut(
            Rc::clone(&self.0.borrow().socket),
            "message",
            move |msg| {
                let parsed = ServerMessage::try_from(&msg)
                    .map(Into::into)
                    .map_err(tracerr::wrap!());
                console_error!(format!(
                    "on_close: {:?}",
                    socket_clone.0.borrow().on_close
                ));
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
        console_error!(format!("{} -- {}", line!(), Rc::strong_count(&self.0)));

        Ok(())
    }
}

impl Drop for InnerSocket {
    fn drop(&mut self) {
        console_error!("Drop WebSocketRpcTransport.");
        if self.socket_state.can_close() {
            self.on_open_listener.take();
            self.on_error_listener.take();
            self.on_message_listener.take();
            self.on_close_listener.take();

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

//! [WebSocket] transport wrapper.
//!
//! [WebSocket]: https://developer.mozilla.org/ru/docs/WebSockets

use std::{cell::RefCell, convert::TryFrom, rc::Rc};

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

/// Errors that may occur when working with [`WebSocketRpcClient`].
///
/// [`WebSocketRpcClient`]: super::WebSocketRpcClient
#[derive(Clone, Debug, Display, JsCaused)]
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
    ParseClientMessage(Rc<serde_json::error::Error>),

    /// Occurs when [`ServerMessage`] cannot be parsed.
    #[display(fmt = "Failed to parse server message: {}", _0)]
    ParseServerMessage(Rc<serde_json::error::Error>),

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
        use TransportError::*;
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

    /// State of [`WebSocketTransport`] connection.
    socket_state: State,

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

    /// Subscribers for [`RpcTransport::on_state_change`] events.
    on_state_change_subs: Vec<mpsc::UnboundedSender<State>>,

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

    /// Updates `socket_state` of this [`InnerSocket`] with the provided
    /// [`State`].
    ///
    /// Sends updated [`State`] to `on_state_change` subscribers.
    /// But, if [`State`] is not changed, then nothing will be sent.
    fn update_socket_state(&mut self, new: &State) {
        if self.socket_state.id() != new.id() {
            self.socket_state = new.clone();
            self.on_state_change_subs.retain(|sub| !sub.is_closed());

            self.on_state_change_subs
                .iter()
                .filter_map(|sub| sub.unbounded_send(new.clone()).err())
                .for_each(|e| {
                    console_error(format!(
                        "'WebSocketRpcTransport::on_state_change' subscriber \
                         has gone unexpectedly: {:?}",
                        e,
                    ));
                });
        }
    }

    /// Checks underlying WebSocket state and updates `socket_state`.
    fn sync_socket_state(&mut self) {
        self.update_socket_state(&self.socket.ready_state().into());
    }
}

impl Drop for InnerSocket {
    fn drop(&mut self) {
        if self.socket_state.can_close() {
            let rsn = serde_json::to_string(&self.close_reason)
                .expect("Could not serialize close message");
            if let Err(e) = self.socket.close_with_code_and_reason(1000, &rsn) {
                console_error(e);
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
/// the [`Drop`] implementation and mention in the list below:
/// 1. [`InnerSocket::on_close_listener`]
/// 2. [`InnerSocket::on_message_listener`]
/// 3. [`InnerSocket::on_open_listener`]
pub struct WebSocketRpcTransport(Rc<RefCell<InnerSocket>>);

impl WebSocketRpcTransport {
    /// Initiates new WebSocket connection. Resolves only when underlying
    /// connection becomes active.
    ///
    /// # Errors
    ///
    /// With [`TransportError::CreateSocket`] if could not open WebSocket to
    /// specified url.
    ///
    /// With [`TransportError::InitSocket`] if [`WebSocket.onclose`][1] callback
    /// fired before [`WebSocket.onopen`][2] callback.
    ///
    /// [1]: https://developer.mozilla.org/en-US/docs/Web/API/WebSocket/onclose
    /// [2]: https://developer.mozilla.org/en-US/docs/Web/API/WebSocket/onopen
    pub async fn new(url: &str) -> Result<Self> {
        let (tx_close, rx_close) = oneshot::channel();
        let (tx_open, rx_open) = oneshot::channel();

        let socket = Rc::new(RefCell::new(InnerSocket::new(url)?));

        {
            let mut socket_mut = socket.borrow_mut();
            let inner = Rc::clone(&socket);
            socket_mut.on_close_listener = Some(
                EventListener::new_once(
                    Rc::clone(&socket_mut.socket),
                    "close",
                    move |_| {
                        inner.borrow_mut().sync_socket_state();
                        let _ = tx_close.send(());
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
                        inner.borrow_mut().sync_socket_state();
                        let _ = tx_open.send(());
                    },
                )
                .map_err(tracerr::map_from_and_wrap!(=> TransportError))?,
            );
        }

        let state = future::select(rx_open, rx_close).await;

        let this = Self(socket);
        this.set_on_close_listener()?;
        this.set_on_message_listener()?;

        match state {
            future::Either::Left((opened, _)) => match opened {
                Ok(_) => Ok(this),
                Err(_) => Err(tracerr::new!(TransportError::InitSocket)),
            },
            future::Either::Right(_) => {
                Err(tracerr::new!(TransportError::InitSocket))
            }
        }
    }

    /// Sets [`WebSocketRpcTransport::on_close_listener`] which will update
    /// [`RpcTransport`]'s [`State`] to [`State::Closed`] with a
    /// [`ClosedStateReason::ConnectionLoss`].
    fn set_on_close_listener(&self) -> Result<()> {
        let this = Rc::clone(&self.0);
        let on_close = EventListener::new_once(
            Rc::clone(&self.0.borrow().socket),
            "close",
            move |msg: CloseEvent| {
                let close_msg = CloseMsg::from(&msg);
                this.borrow_mut().update_socket_state(&State::Closed(
                    ClosedStateReason::ConnectionLost(close_msg),
                ));
            },
        )
        .map_err(tracerr::map_from_and_wrap!(=> TransportError))?;
        self.0.borrow_mut().on_close_listener = Some(on_close);

        Ok(())
    }

    /// Sets [`WebSocketRpcTransport::on_message_listener`] which will send
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
                this_mut
                    .on_message_subs
                    .retain(|on_message| !on_message.is_closed());
                this_mut.on_message_subs.iter().for_each(|on_message| {
                    on_message.unbounded_send(msg.clone()).unwrap_or_else(
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

    fn on_state_change(&self) -> LocalBoxStream<'static, State> {
        let (tx, rx) = mpsc::unbounded();
        self.0.borrow_mut().on_state_change_subs.push(tx);

        Box::pin(rx)
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

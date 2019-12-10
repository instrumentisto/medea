//! Abstraction over RPC transport.

mod heartbeat;
mod websocket;

use std::{
    cell::RefCell,
    rc::{Rc, Weak},
    vec,
};

use derive_more::{Display, From};
use futures::{
    channel::{mpsc, oneshot},
    future::LocalBoxFuture,
    stream::{LocalBoxStream, StreamExt as _},
};
use js_sys::{Date, Promise};
use medea_client_api_proto::{
    ClientMsg, CloseDescription, CloseReason as CloseByServerReason, Command,
    Event, ServerMsg,
};
use serde::Serialize;
use tracerr::Traced;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::CloseEvent;

use crate::utils::{window, JasonError, JsCaused, JsError};

use self::heartbeat::{Heartbeat, HeartbeatError};

#[doc(inline)]
pub use self::websocket::{TransportError, WebSocketRpcTransport};

/// Reasons of closing by client side and server side.
#[derive(Copy, Clone, Display, Debug, Eq, PartialEq)]
pub enum CloseReason {
    /// Closed by server.
    ByServer(CloseByServerReason),

    /// Closed by client.
    #[display(fmt = "{}", reason)]
    ByClient {
        /// Reason of closing.
        reason: ClientDisconnect,

        /// Is closing considered as error.
        is_err: bool,
    },
}

/// Reasons of closing by client side.
#[derive(Copy, Clone, Display, Debug, Eq, PartialEq, Serialize)]
pub enum ClientDisconnect {
    /// [`Room`] was dropped without `close_reason`.
    RoomUnexpectedlyDropped,

    /// [`Room`] was normally closed by JS side.
    RoomClosed,

    /// [`RpcClient`] was unexpectedly dropped.
    RpcClientUnexpectedlyDropped,

    /// [`RpcTransport`] was unexpectedly dropped.
    RpcTransportUnexpectedlyDropped,
}

impl ClientDisconnect {
    /// Returns `true` if [`CloseByClientReason`] is considered as error.
    pub fn is_err(self) -> bool {
        match self {
            Self::RoomUnexpectedlyDropped
            | Self::RpcClientUnexpectedlyDropped
            | Self::RpcTransportUnexpectedlyDropped => true,
            Self::RoomClosed => false,
        }
    }
}

impl Into<CloseReason> for ClientDisconnect {
    fn into(self) -> CloseReason {
        CloseReason::ByClient {
            is_err: self.is_err(),
            reason: self,
        }
    }
}

/// Connection with remote was closed.
#[derive(Debug)]
pub enum CloseMsg {
    /// Transport was gracefully closed by remote.
    ///
    /// Determines by close code `1000` and existence of
    /// [`RpcConnectionCloseReason`].
    Normal(u16, CloseByServerReason),

    /// Connection was unexpectedly closed. Consider reconnecting.
    ///
    /// Unexpected close determines by non-`1000` close code and for close code
    /// `1000` without reason.
    Abnormal(u16),
}

impl From<&CloseEvent> for CloseMsg {
    fn from(event: &CloseEvent) -> Self {
        let code: u16 = event.code();
        match code {
            1000 => {
                if let Ok(description) =
                    serde_json::from_str::<CloseDescription>(&event.reason())
                {
                    Self::Normal(code, description.reason)
                } else {
                    Self::Abnormal(code)
                }
            }
            _ => Self::Abnormal(code),
        }
    }
}

/// Errors that may occur in [`RpcClient`].
#[derive(Debug, Display, From, JsCaused)]
#[allow(clippy::module_name_repetitions)]
pub enum RpcClientError {
    /// Occurs if WebSocket connection to remote media server failed.
    #[display(fmt = "Connection failed: {}", _0)]
    RpcTransportError(#[js(cause)] TransportError),

    /// Occurs if the heartbeat cannot be started.
    #[display(fmt = "Start heartbeat failed: {}", _0)]
    CouldNotStartHeartbeat(#[js(cause)] HeartbeatError),

    /// Occurs if [`ProgressiveDelay`] fails to set timeout.
    #[display(fmt = "Failed to set JS timeout: {}", _0)]
    SetTimeoutError(#[js(cause)] JsError),

    #[display(fmt = "Socket is None.")]
    NoSocket,
}

// TODO: consider using async-trait crate, it doesnt work with mockall atm
/// Client to talk with server via Client API RPC.
#[allow(clippy::module_name_repetitions)]
#[cfg_attr(feature = "mockable", mockall::automock)]
pub trait RpcClient {
    /// Establishes connection with RPC server.
    fn connect(
        &self,
        transport: Rc<dyn RpcTransport>,
    ) -> LocalBoxFuture<'static, Result<(), Traced<RpcClientError>>>;

    /// Returns [`Stream`] of all [`Event`]s received by this [`RpcClient`].
    ///
    /// [`Stream`]: futures::Stream
    fn subscribe(&self) -> LocalBoxStream<'static, Event>;

    /// Unsubscribes from this [`RpcClient`]. Drops all subscriptions atm.
    fn unsub(&self);

    /// Sends [`Command`] to server.
    fn send_command(&self, command: Command);

    /// Returns [`Future`] which will be resolved with [`CloseReason`] on
    /// RPC connection close, caused by underlying transport close. Will not be
    /// invoked on [`RpcClient`] drop.
    fn on_close(
        &self,
    ) -> LocalBoxFuture<'static, Result<CloseReason, oneshot::Canceled>>;

    /// Sets reason, that will be passed to underlying transport when this
    /// client will be dropped.
    fn set_close_reason(&self, close_reason: ClientDisconnect);
}

/// RPC transport between client and server.
#[allow(clippy::module_name_repetitions)]
#[cfg_attr(feature = "mockable", mockall::automock)]
pub trait RpcTransport {
    /// Returns [`LocalBoxStream`] of all messages received by this transport.
    fn on_message(
        &self,
    ) -> Result<
        LocalBoxStream<'static, Result<ServerMsg, Traced<TransportError>>>,
        Traced<TransportError>,
    >;

    /// Returns [`LocalBoxFuture`], that will be resolved when this transport
    /// will be closed.
    fn on_close(
        &self,
    ) -> Result<LocalBoxStream<'static, CloseMsg>, Traced<TransportError>>;

    /// Sets reason, that will be sent to remote server when this transport will
    /// be dropped.
    fn set_close_reason(&self, reason: ClientDisconnect);

    /// Sends message to server.
    fn send(&self, msg: &ClientMsg) -> Result<(), Traced<TransportError>>;

    /// Try to reconnect [`RpcTransport`].
    fn reconnect(
        &self,
    ) -> LocalBoxFuture<'static, Result<(), Traced<TransportError>>>;
}

/// Inner state of [`WebsocketRpcClient`].
struct Inner {
    /// [`WebSocket`] connection to remote media server.
    sock: Option<Rc<dyn RpcTransport>>,

    heartbeat: Heartbeat,

    /// Event's subscribers list.
    subs: Vec<mpsc::UnboundedSender<Event>>,

    /// [`oneshot::Sender`] with which [`CloseReason`] will be sent when
    /// WebSocket connection normally closed by server.
    ///
    /// Note that [`CloseReason`] will not be sent if WebSocket closed with
    /// [`RpcConnectionCloseReason::NewConnection`] reason.
    on_close_subscribers: Vec<oneshot::Sender<CloseReason>>,

    /// Reason of [`WebsocketRpcClient`] closing.
    ///
    /// This reason will be provided to underlying [`RpcTransport`].
    close_reason: ClientDisconnect,

    /// Indicates that this [`WebSocketRpcClient`] is closed.
    is_closed: bool,
}

impl Inner {
    fn new(heartbeat_interval: i32) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            sock: None,
            on_close_subscribers: Vec::new(),
            subs: vec![],
            heartbeat: Heartbeat::new(heartbeat_interval),
            close_reason: ClientDisconnect::RpcClientUnexpectedlyDropped,
            is_closed: false,
        }))
    }
}

pub struct WeakWebsocketRpcClient(Weak<RefCell<Inner>>);

impl WeakWebsocketRpcClient {
    pub fn new(strong_pointer: WebSocketRpcClient) -> Self {
        Self(Rc::downgrade(&strong_pointer.0))
    }

    pub fn upgrade(&self) -> WebSocketRpcClient {
        WebSocketRpcClient(self.0.upgrade().unwrap())
    }
}

/// Handles close message from a remote server.
///
/// This function will be called on every WebSocket close (normal and abnormal)
/// regardless of the [`CloseReason`].
async fn on_close(client: WebSocketRpcClient, close_msg: &CloseMsg) {
    client.0.borrow_mut().heartbeat.stop();

    // TODO: propagate error if unable to reconnect

    match &close_msg {
        CloseMsg::Normal(_, reason) => match reason {
            CloseByServerReason::Reconnected => (),
            CloseByServerReason::Idle => {
                client.reconnect().await;
            }
            _ => {
                client.0.borrow_mut().sock.take();
                if *reason != CloseByServerReason::Reconnected {
                    client
                        .0
                        .borrow_mut()
                        .on_close_subscribers
                        .drain(..)
                        .filter_map(|sub| {
                            sub.send(CloseReason::ByServer(*reason)).err()
                        })
                        .for_each(|reason| {
                            console_error!(format!(
                                "Failed to send reason of Jason close to \
                                 subscriber: {:?}",
                                reason
                            ))
                        });
                }
            }
        },
        CloseMsg::Abnormal(_) => spawn_local(async move {
            client.reconnect().await;
        }),
    }
}

/// Handles messages from a remote server.
fn on_message(
    client: &WebSocketRpcClient,
    msg: Result<ServerMsg, Traced<TransportError>>,
) {
    let inner = client.0.borrow();
    match msg {
        Ok(ServerMsg::Pong(_num)) => {
            // TODO: detect no pings
            inner.heartbeat.set_pong_at(Date::now());
        }
        Ok(ServerMsg::Event(event)) => {
            // TODO: many subs, filter messages by session
            if let Some(sub) = inner.subs.iter().next() {
                if let Err(err) = sub.unbounded_send(event) {
                    // TODO: receiver is gone, should delete
                    //       this subs tx
                    console_error!(err.to_string());
                }
            }
        }
        Err(err) => {
            // TODO: protocol versions mismatch? should drop
            //       connection if so
            JasonError::from(err).print();
        }
    }
}

struct ProgressiveDelay {
    current_delay: i32,
}

impl ProgressiveDelay {
    const MAX_DELAY: i32 = 10000;

    /// Returns [`ProgressiveDelay`] with first delay as `1250ms`.
    pub fn new() -> Self {
        Self {
            current_delay: 1250,
        }
    }

    /// Returns next step of delay.
    fn get_delay(&mut self) -> i32 {
        if self.is_max_delay_reached() {
            Self::MAX_DELAY
        } else {
            self.current_delay *= 2;
            self.current_delay
        }
    }

    /// Resolves after next delay.
    pub async fn delay(&mut self) -> Result<(), Traced<JsError>> {
        let delay_ms = self.get_delay();
        JsFuture::from(Promise::new(&mut |yes, _| {
            window()
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    &yes, delay_ms,
                )
                .unwrap();
        }))
        .await
        .map(|_| ())
        .map_err(JsError::from)
        .map_err(tracerr::wrap!())
    }

    /// Returns `true` when `current_delay > Self::MAX_DELAY`.
    fn is_max_delay_reached(&self) -> bool {
        self.current_delay >= Self::MAX_DELAY
    }
}

// TODO:
// 1. Proper sub registry.
// 2. Reconnect.
// 3. Disconnect if no pongs.
// 4. Buffering if no socket?
/// Client API RPC client to talk with server via [`WebSocket`].
#[allow(clippy::module_name_repetitions)]
#[derive(Clone)]
pub struct WebSocketRpcClient(Rc<RefCell<Inner>>);

impl WebSocketRpcClient {
    /// Creates new [`WebsocketRpcClient`] with a given `ping_interval` in
    /// milliseconds.
    pub fn new(ping_interval: i32) -> Self {
        Self(Inner::new(ping_interval))
    }

    /// Reconnect [`WebSocketRpcClient`].
    pub async fn reconnect(&self) -> Result<(), Traced<RpcClientError>> {
        let mut delayer = ProgressiveDelay::new();
        while let Err(_) = self
            .0
            .borrow()
            .sock
            .as_ref()
            .ok_or_else(|| tracerr::new!(RpcClientError::NoSocket))?
            .reconnect()
            .await
        {
            // TODO (evdokimovs): `.map_err(RpcClientError::SetTimeoutError)`
            // will be much better, but I don't know how achieve it with
            // `tracerr` crate.
            delayer
                .delay()
                .await
                .map_err(tracerr::map_from_and_wrap!())?;
        }
        let sock = self.0.borrow().sock.clone();
        if let Some(sock) = sock {
            self.0
                .borrow_mut()
                .heartbeat
                .start(sock)
                .map_err(tracerr::map_from_and_wrap!())?;
        }
        Ok(())
    }
}

impl RpcClient for WebSocketRpcClient {
    /// Creates new WebSocket connection to remote media server.
    /// Starts `Heartbeat` if connection succeeds and binds handlers
    /// on receiving messages from server and closing socket.
    fn connect(
        &self,
        transport: Rc<dyn RpcTransport>,
    ) -> LocalBoxFuture<'static, Result<(), Traced<RpcClientError>>> {
        let this = self.clone();
        Box::pin(async move {
            this.0
                .borrow_mut()
                .heartbeat
                .start(Rc::clone(&transport))
                .map_err(tracerr::map_from_and_wrap!())?;

            let this_clone = WeakWebsocketRpcClient::new(this.clone());
            let mut on_socket_message = transport
                .on_message()
                .map_err(tracerr::map_from_and_wrap!())?;
            spawn_local(async move {
                while let Some(msg) = on_socket_message.next().await {
                    on_message(&this_clone.upgrade(), msg)
                }
            });

            let this_clone = WeakWebsocketRpcClient::new(this.clone());
            let mut on_socket_close = transport
                .on_close()
                .map_err(tracerr::map_from_and_wrap!())?;
            spawn_local(async move {
                while let Some(msg) = on_socket_close.next().await {
                    on_close(this_clone.upgrade().clone(), &msg).await;
                }
            });

            this.0.borrow_mut().sock.replace(transport);
            Ok(())
        })
    }

    /// Returns [`Stream`] of all [`Event`]s received by this [`RpcClient`].
    ///
    /// [`Stream`]: futures::Stream
    // TODO: proper sub registry
    fn subscribe(&self) -> LocalBoxStream<'static, Event> {
        let (tx, rx) = mpsc::unbounded();
        self.0.borrow_mut().subs.push(tx);

        Box::pin(rx)
    }

    /// Unsubscribes from this [`RpcClient`]. Drops all subscriptions atm.
    // TODO: proper sub registry
    fn unsub(&self) {
        self.0.borrow_mut().subs.clear();
    }

    /// Sends [`Command`] to RPC server.
    // TODO: proper sub registry
    fn send_command(&self, command: Command) {
        let socket_borrow = &self.0.borrow().sock;

        // TODO: no socket? we dont really want this method to return err
        if let Some(socket) = socket_borrow.as_ref() {
            socket.send(&ClientMsg::Command(command)).unwrap();
        }
    }

    /// Returns [`Future`] which will be resolved with [`CloseReason`] on
    /// RPC connection close, caused by underlying transport close. Will not be
    /// invoked on [`RpcClient`] drop.
    fn on_close(
        &self,
    ) -> LocalBoxFuture<'static, Result<CloseReason, oneshot::Canceled>> {
        let (tx, rx) = oneshot::channel();
        self.0.borrow_mut().on_close_subscribers.push(tx);
        Box::pin(rx)
    }

    /// Sets reason, that will be passed to underlying transport when this
    /// client will be dropped.
    fn set_close_reason(&self, close_reason: ClientDisconnect) {
        self.0.borrow_mut().close_reason = close_reason
    }
}

impl Drop for Inner {
    /// Drops related connection and its [`Heartbeat`].
    fn drop(&mut self) {
        self.is_closed = true;
        if let Some(socket) = self.sock.take() {
            socket.set_close_reason(self.close_reason.clone());
        }
        self.heartbeat.stop();
    }
}

//! Abstraction over RPC transport.

mod backoff_delayer;
mod heartbeat;
mod reconnect_handle;
mod websocket;

use std::{cell::RefCell, rc::Rc, time::Duration, vec};

use derive_more::{Display, From};
use futures::{
    channel::{mpsc, oneshot},
    future::LocalBoxFuture,
    stream::{LocalBoxStream, StreamExt as _},
};
use medea_client_api_proto::{
    ClientMsg, CloseDescription, CloseReason as CloseByServerReason, Command,
    Event, ServerMsg,
};
use serde::Serialize;
use tracerr::Traced;
use wasm_bindgen_futures::spawn_local;
use web_sys::CloseEvent;

use crate::utils::{console_error, JsCaused, JsError};

#[cfg(not(feature = "mockable"))]
use self::{
    backoff_delayer::{BackoffDelayer, BackoffDelayerError},
    heartbeat::{Heartbeat, HeartbeatError},
};

#[cfg(feature = "mockable")]
pub use self::{
    backoff_delayer::{BackoffDelayer, BackoffDelayerError},
    heartbeat::{Heartbeat, HeartbeatError},
};
#[doc(inline)]
pub use self::{
    heartbeat::{IdleTimeout, PingInterval},
    reconnect_handle::ReconnectorHandle,
    websocket::{State, TransportError, WebSocketRpcTransport},
};

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
#[derive(Clone, Debug)]
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
pub enum RpcClientError {
    /// Occurs if WebSocket connection to remote media server failed.
    #[display(fmt = "Connection failed: {}", _0)]
    RpcTransportError(#[js(cause)] TransportError),

    /// Occurs if the heartbeat cannot be started.
    #[display(fmt = "Start heartbeat failed: {}", _0)]
    CouldNotStartHeartbeat(#[js(cause)] HeartbeatError),

    /// Occurs if `socket` of [`WebSocketRpcClient`] is unexpectedly `None`.
    #[display(fmt = "Socket of 'WebSocketRpcClient' is unexpectedly 'None'.")]
    NoSocket,

    /// Occurs if [`BackoffDelayer`] errored.
    BackoffDelayer(#[js(cause)] BackoffDelayerError),

    /// Occurs if [`Weak`] pointer to the [`RpcClient`] can't be upgraded to
    /// [`Rc`].
    #[display(fmt = "RpcClient unexpectedly gone.")]
    RpcClientGone,

    /// Occurs if reconnection performed earlier was failed. We can't provide
    /// concrete reason because we determine it by subscribing to the
    /// [`RpcTransport::on_state_change`].
    #[display(fmt = "Reconnection failed.")]
    ReconnectionFailed,

    FirstServerMsgIsNotRpcSettings,
}

// TODO: consider using async-trait crate, it doesnt work with mockall atm
/// Client to talk with server via Client API RPC.
#[cfg_attr(feature = "mockable", mockall::automock)]
pub trait RpcClient {
    /// Establishes connection with RPC server.
    fn connect(
        &self,
        token: String,
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

    // TODO: whats the difference between this and on_close?
    //       requires better naming and documentation

    /// Returns [`Stream`] to which will be sent [`ReconnectHandle`] (with which
    /// JS side can perform reconnection) on all connection losses.
    fn on_connection_loss(&self) -> LocalBoxStream<'static, ()>;

    fn get_token(&self) -> Option<String>;

    fn get_state(&self) -> State;

    fn on_state_change(&self) -> LocalBoxStream<'static, State>;
}

/// RPC transport between a client and server.
#[cfg_attr(feature = "mockable", mockall::automock)]
pub trait RpcTransport {
    /// Returns [`LocalBoxStream`] of all messages received by this transport.
    fn on_message(
        &self,
    ) -> Result<LocalBoxStream<'static, ServerMsg>, Traced<TransportError>>;

    /// Returns [`LocalBoxStream`] which will produce [`CloseMsg`]s on
    /// [`RpcTransport`] close. This is [`LocalBoxStream`] because
    /// [`RpcTransport`] can reconnect after closing with
    /// [`RpcTransport::reconnect`].
    fn on_close(
        &self,
    ) -> Result<LocalBoxStream<'static, CloseMsg>, Traced<TransportError>>;

    /// Sets reason, that will be sent to remote server when this transport will
    /// be dropped.
    fn set_close_reason(&self, reason: ClientDisconnect);

    /// Sends a message to server.
    fn send(&self, msg: &ClientMsg) -> Result<(), Traced<TransportError>>;

    /// Returns [`State`] of underlying [`RpcTransport`]'s
    /// connection.
    fn get_state(&self) -> State;

    /// Subscribes to the [`State`] changes.
    ///
    /// This function guarantees that two identical [`State`]s in a row doesn't
    /// will be sent.
    fn on_state_change(&self) -> LocalBoxStream<'static, State>;
}

/// Inner state of [`WebsocketRpcClient`].
struct Inner {
    /// [`WebSocket`] connection to remote media server.
    sock: Option<Rc<dyn RpcTransport>>,

    /// Service for sending/receiving ping pongs between the client and server.
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

    /// Senders for [`RpcClient::on_connection_loss`].
    on_connection_loss_subs: Vec<mpsc::UnboundedSender<()>>,

    rpc_transport_factory: RpcTransportFactory,

    token: Option<String>,

    on_state_change_subs: Vec<mpsc::UnboundedSender<State>>,

    state: State,
}

type RpcTransportFactory = Box<
    dyn Fn(
        String,
    ) -> LocalBoxFuture<
        'static,
        Result<Rc<dyn RpcTransport>, Traced<TransportError>>,
    >,
>;

impl Inner {
    fn new(rpc_transport_factory: RpcTransportFactory) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            sock: None,
            on_close_subscribers: Vec::new(),
            subs: vec![],
            heartbeat: Heartbeat::new(),
            close_reason: ClientDisconnect::RpcClientUnexpectedlyDropped,
            on_connection_loss_subs: Vec::new(),
            rpc_transport_factory,
            token: None,
            on_state_change_subs: Vec::new(),
            state: State::Closed,
        }))
    }

    fn update_state(&mut self, state: State) {
        if self.state != state {
            self.state = state;
            self.on_state_change_subs.retain(|sub| !sub.is_closed());
            self.on_state_change_subs
                .iter()
                .filter_map(|sub| sub.unbounded_send(state).err())
                .for_each(|_| {
                    console_error(
                        "RpcClient::on_state_change sub unexpectedly gone.",
                    )
                });
        }
    }
}

// TODO:
// 1. Proper sub registry.
// 2. Reconnect.
// 3. Disconnect if no pongs.
// 4. Buffering if no socket?
/// Client API RPC client to talk with a server via [`WebSocket`].
///
/// Don't derive [`Clone`] and don't use it if there are no very serious reasons
/// for this. Because with many strong [`Rc`]s we can catch many painful bugs
/// with [`Drop`] implementation, memory leaks etc. It is especially not
/// recommended use a strong pointer ([`Rc`]) in all kinds of callbacks and
/// `async` closures. If you clone this then make sure that this
/// [`WebSocketRpcClient`] will be normally [`Drop`]ed.
///
/// Alternative for [`Clone`] is [`WebSocketRpcClient::downgrade`] which will
/// return [`WeakWebSocketRpcClient`] which can be upgraded to
/// [`WebSocketRpcClient`] and will not hold this structure from destruction.
pub struct WebSocketRpcClient(Rc<RefCell<Inner>>);

impl WebSocketRpcClient {
    /// Creates new [`WebsocketRpcClient`] with a given `ping_interval` in
    /// milliseconds.
    pub fn new(rpc_transport_factory: RpcTransportFactory) -> Self {
        Self(Inner::new(rpc_transport_factory))
    }

    /// Stops [`Heartbeat`], sends [`ReconnectHandle`] to all
    /// [`RpcClient::on_connection_loss`] subs
    fn send_connection_loss(&self) {
        self.0.borrow_mut().heartbeat.stop();
        self.0
            .borrow_mut()
            .on_connection_loss_subs
            .retain(|sub| !sub.is_closed());

        let inner = self.0.borrow();
        for sub in &inner.on_connection_loss_subs {
            if sub.unbounded_send(()).is_err() {
                console_error(
                    "RpcClient::on_connection_loss subscriber is unexpectedly \
                     gone.",
                );
            }
        }
    }

    /// Handles close message from a remote server.
    ///
    /// This function will be called on every WebSocket close (normal and
    /// abnormal) regardless of the [`CloseReason`].
    fn on_transport_close(&self, close_msg: &CloseMsg) {
        self.0.borrow_mut().heartbeat.stop();

        match &close_msg {
            CloseMsg::Normal(_, reason) => match reason {
                CloseByServerReason::Reconnected => (),
                CloseByServerReason::Idle => {
                    self.send_connection_loss();
                }
                _ => {
                    self.0.borrow_mut().sock.take();
                    if *reason != CloseByServerReason::Reconnected {
                        self.0
                            .borrow_mut()
                            .on_close_subscribers
                            .drain(..)
                            .filter_map(|sub| {
                                sub.send(CloseReason::ByServer(*reason)).err()
                            })
                            .for_each(|reason| {
                                console_error(format!(
                                    "Failed to send reason of Jason close to \
                                     subscriber: {:?}",
                                    reason
                                ))
                            });
                    }
                }
            },
            CloseMsg::Abnormal(_) => {
                self.send_connection_loss();
            }
        }
    }

    /// Handles messages from a remote server.
    fn on_transport_message(&self, msg: ServerMsg) {
        match msg {
            ServerMsg::Event(event) => {
                let inner = self.0.borrow();
                // TODO: many subs, filter messages by session
                if let Some(sub) = inner.subs.iter().next() {
                    if let Err(err) = sub.unbounded_send(event) {
                        // TODO: receiver is gone, should delete
                        //       this subs tx
                        console_error(err.to_string());
                    }
                }
            }
            ServerMsg::RpcSettingsUpdated(settings) => {
                self.update_settings(
                    IdleTimeout(
                        Duration::from_millis(settings.idle_timeout_ms).into(),
                    ),
                    PingInterval(
                        Duration::from_millis(settings.ping_interval_ms).into(),
                    ),
                );
            }
            _ => (),
        }
    }

    async fn connect(
        &self,
        token: String,
    ) -> Result<(), Traced<RpcClientError>> {
        self.0.borrow_mut().token = Some(token.clone());
        self.0.borrow_mut().update_state(State::Connecting);
        let create_transport_fut =
            (self.0.borrow().rpc_transport_factory)(token);
        let transport = create_transport_fut
            .await
            .map_err(tracerr::map_from_and_wrap!())
            .map_err(|e| {
                self.0.borrow_mut().update_state(State::Closed);
                e
            })?;

        if let Some(msg) = transport
            .on_message()
            .map_err(tracerr::map_from_and_wrap!())?
            .next()
            .await
        {
            if let ServerMsg::RpcSettingsUpdated(rpc_settings) = msg {
                let idle_timeout = IdleTimeout(
                    Duration::from_millis(rpc_settings.idle_timeout_ms).into(),
                );
                let ping_interval = PingInterval(
                    Duration::from_millis(rpc_settings.ping_interval_ms).into(),
                );
                self.0
                    .borrow_mut()
                    .heartbeat
                    .start(idle_timeout, ping_interval, Rc::clone(&transport))
                    .map_err(tracerr::map_from_and_wrap!())?;
                let mut on_idle = self.0.borrow().heartbeat.on_idle();
                let weak_this = Rc::downgrade(&self.0);
                spawn_local(async move {
                    while let Some(_) = on_idle.next().await {
                        if let Some(this) = weak_this.upgrade().map(Self) {
                            this.send_connection_loss();
                        }
                    }
                });
            } else {
                return Err(tracerr::new!(
                    RpcClientError::FirstServerMsgIsNotRpcSettings
                ));
            }
        } else {
            return Err(tracerr::new!(RpcClientError::NoSocket));
        }

        self.0.borrow_mut().update_state(State::Open);
        let mut transport_on_state_change_stream = transport.on_state_change();
        let weak_inner = Rc::downgrade(&self.0);
        spawn_local(async move {
            while let Some(state) =
                transport_on_state_change_stream.next().await
            {
                if let Some(inner) = weak_inner.upgrade() {
                    inner.borrow_mut().update_state(state);
                }
            }
        });

        let this_clone = Rc::downgrade(&self.0);
        let mut on_socket_message = transport
            .on_message()
            .map_err(tracerr::map_from_and_wrap!())?;
        spawn_local(async move {
            while let Some(msg) = on_socket_message.next().await {
                if let Some(this) = this_clone.upgrade().map(Self) {
                    this.on_transport_message(msg)
                }
            }
        });

        let this_clone = Rc::downgrade(&self.0);
        let mut on_socket_close = transport
            .on_close()
            .map_err(tracerr::map_from_and_wrap!())?;
        spawn_local(async move {
            while let Some(msg) = on_socket_close.next().await {
                if let Some(this) = this_clone.upgrade().map(Self) {
                    this.on_transport_close(&msg);
                }
            }
        });

        self.0.borrow_mut().sock.replace(transport);
        Ok(())
    }

    /// Updates RPC settings of this [`RpcClient`].
    fn update_settings(
        &self,
        idle_timeout: IdleTimeout,
        ping_interval: PingInterval,
    ) {
        self.0
            .borrow_mut()
            .heartbeat
            .update_settings(idle_timeout, ping_interval);
    }
}

impl RpcClient for WebSocketRpcClient {
    /// Creates new WebSocket connection to remote media server.
    /// Starts `Heartbeat` if connection succeeds and binds handlers
    /// on receiving messages from a server and closing socket.
    fn connect(
        &self,
        token: String,
    ) -> LocalBoxFuture<'static, Result<(), Traced<RpcClientError>>> {
        let this = Self(Rc::clone(&self.0));
        Box::pin(async move {
            let current_token = this.0.borrow().token.clone();
            if let Some(current_token) = current_token {
                if current_token == token {
                    let state = this.get_state();
                    match state {
                        State::Open => Ok(()),
                        State::Connecting => {
                            let mut transport_state_stream =
                                this.on_state_change();
                            while let Some(state) =
                                transport_state_stream.next().await
                            {
                                match state {
                                    State::Open => {
                                        return Ok(());
                                    }
                                    State::Closing | State::Closed => {
                                        // Change error
                                        return Err(tracerr::new!(
                                            RpcClientError::ReconnectionFailed
                                        ));
                                    }
                                    State::Connecting => (),
                                }
                            }
                            // TODO: PANIC
                            panic!("RpcTransport unexpectedly gone.")
                        }
                        State::Closed | State::Closing => {
                            this.connect(token).await
                        }
                    }
                } else {
                    this.connect(token).await
                }
            } else {
                this.connect(token).await
            }
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

    /// Returns [`LocalBoxStream`] to which will be sent
    /// [`ReconnectionHandle`] on connection losing.
    fn on_connection_loss(&self) -> LocalBoxStream<'static, ()> {
        let (tx, rx) = mpsc::unbounded();
        self.0.borrow_mut().on_connection_loss_subs.push(tx);

        Box::pin(rx)
    }

    fn get_token(&self) -> Option<String> {
        self.0.borrow().token.clone()
    }

    fn get_state(&self) -> State {
        self.0.borrow().state
    }

    fn on_state_change(&self) -> LocalBoxStream<'static, State> {
        let (tx, rx) = mpsc::unbounded();
        self.0.borrow_mut().on_state_change_subs.push(tx);

        Box::pin(rx)
    }
}

impl Drop for Inner {
    /// Drops related connection and its [`Heartbeat`].
    fn drop(&mut self) {
        if let Some(socket) = self.sock.take() {
            socket.set_close_reason(self.close_reason.clone());
        }
        self.heartbeat.stop();
    }
}

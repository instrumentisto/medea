//! Abstraction over RPC transport.

mod backoff_delayer;
mod heartbeat;
mod reconnect_handle;
mod websocket;

use std::{
    cell::RefCell,
    rc::{Rc, Weak},
    time::Duration,
    vec,
};

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

use crate::utils::{console_error, JsCaused, JsDuration, JsError};

use self::reconnect_handle::Reconnector;
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

    /// Reconnection timeout was expired. Indicates that reconnection is
    /// impossible now.
    ReconnectTimeout,
}

impl ClientDisconnect {
    /// Returns `true` if [`CloseByClientReason`] is considered as error.
    pub fn is_err(self) -> bool {
        match self {
            Self::RoomUnexpectedlyDropped
            | Self::RpcClientUnexpectedlyDropped
            | Self::RpcTransportUnexpectedlyDropped
            | Self::ReconnectTimeout => true,
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

    /// Occurs if [`ProgressiveDelayer`] fails to set timeout.
    #[display(fmt = "Failed to set JS timeout: {}", _0)]
    SetTimeoutError(#[js(cause)] JsError),

    /// Occurs if `socket` of [`WebSocketRpcClient`] is unexpectedly `None`.
    #[display(fmt = "Socket of 'WebSocketRpcClient' is unexpectedly 'None'.")]
    NoSocket,

    /// Occurs if [`ProgressiveDelayer`] errored.
    ProgressiveDelayer(#[js(cause)] BackoffDelayerError),

    /// Occurs if time frame in which we can reconnect was passed.
    #[display(fmt = "Reconnection deadline passed.")]
    Deadline,

    /// Occurs if [`Weak`] pointer to the [`RpcClient`] can't be upgraded to
    /// [`Rc`].
    #[display(fmt = "RpcClient unexpectedly gone.")]
    RpcClientGone,

    /// Occurs if reconnection performed earlier was failed. We can't provide
    /// concrete reason because we determine it by subscribing to the
    /// [`RpcTransport::on_state_change`].
    #[display(fmt = "Reconnection failed.")]
    ReconnectionFailed,
}

// TODO: consider using async-trait crate, it doesnt work with mockall atm
/// Client to talk with server via Client API RPC.
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

    /// Updates RPC settings of this [`RpcClient`].
    fn update_settings(
        &self,
        idle_timeout: IdleTimeout,
        ping_interval: PingInterval,
    );

    /// Returns [`Stream`] to which will be sent [`ReconnectHandle`] (with which
    /// JS side can perform reconnection) on all connection losses.
    fn on_connection_loss(&self) -> LocalBoxStream<'static, ReconnectorHandle>;
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

    /// Tries to reconnect this [`RpcTransport`].
    fn reconnect(
        &self,
    ) -> LocalBoxFuture<'static, Result<(), Traced<TransportError>>>;

    /// Returns [`State`] of underlying [`RpcTransport`]'s
    /// connection.
    fn get_state(&self) -> State;

    /// Subscribes to the [`State`] changes.
    ///
    /// This function guarantees that two identical [`State`]s in a row doesn't
    /// will be thrown.
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
    on_connection_loss_sub: Option<mpsc::UnboundedSender<ReconnectorHandle>>,

    /// [`Reconnector`] with which this [`RpcClient`] will be reconnected (or
    /// not) on `on_connection_loss`.
    reconnector: Option<Reconnector>,
}

impl Inner {
    fn new() -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            sock: None,
            on_close_subscribers: Vec::new(),
            subs: vec![],
            heartbeat: Heartbeat::new(
                IdleTimeout(Duration::from_secs(10).into()),
                PingInterval(Duration::from_secs(3).into()),
            ),
            close_reason: ClientDisconnect::RpcClientUnexpectedlyDropped,
            on_connection_loss_sub: None,
            reconnector: None,
        }))
    }
}

/// [`Weak`] pointer which can be upgraded to [`WebSocketRpcClient`].
pub struct WeakWebsocketRpcClient(Weak<RefCell<Inner>>);

impl WeakWebsocketRpcClient {
    /// Returns [`WeakWebSocketRpcClient`] with [`Weak`] pointer to a provided
    /// [`WebSocketRpcClient`].
    pub fn new(strong: &WebSocketRpcClient) -> Self {
        Self(Rc::downgrade(&strong.0))
    }

    /// Returns `Some(WebSocketRpcClient)` if it still exists.
    pub fn upgrade(&self) -> Option<WebSocketRpcClient> {
        self.0.upgrade().map(WebSocketRpcClient)
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
    pub fn new() -> Rc<Self> {
        let rc_this = Rc::new(Self(Inner::new()));
        let weak_this = Rc::downgrade(&rc_this);
        rc_this.0.borrow_mut().reconnector = Some(Reconnector::new(weak_this));

        rc_this
    }

    /// Stops [`Heartbeat`], sends [`ReconnectHandle`] to all
    /// [`RpcClient::on_connection_loss`] subs
    fn send_connection_loss(&self) {
        self.0.borrow_mut().heartbeat.stop();
        if let Some(on_connection_loss) =
            &self.0.borrow().on_connection_loss_sub
        {
            let handle =
                self.0.borrow().reconnector.as_ref().unwrap().new_handle();
            if on_connection_loss.unbounded_send(handle).is_err() {
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
        let inner = self.0.borrow();
        if let ServerMsg::Event(event) = msg {
            // TODO: many subs, filter messages by session
            if let Some(sub) = inner.subs.iter().next() {
                if let Err(err) = sub.unbounded_send(event) {
                    // TODO: receiver is gone, should delete
                    //       this subs tx
                    console_error(err.to_string());
                }
            }
        }
    }

    /// Downgrades strong ([`Rc`]) pointed [`WebSocketRpcClient`] to a [`Weak`]
    /// pointed [`WeakWebSocketRpcClient`].
    fn downgrade(&self) -> WeakWebsocketRpcClient {
        WeakWebsocketRpcClient::new(self)
    }

    /// Tries to reconnect [`RpcTransport`].
    ///
    /// If reconnection is successful then [`Heartbeat`] of this [`RpcClient`]
    /// will be started again with reconnected [`RpcTransport`].
    async fn try_reconnect(
        &self,
        sock: &Rc<dyn RpcTransport>,
    ) -> Result<(), Traced<RpcClientError>> {
        sock.reconnect()
            .await
            .map_err(tracerr::map_from_and_wrap!())?;

        let transport = self.0.borrow().sock.clone();
        if let Some(transport) = transport {
            self.0
                .borrow_mut()
                .heartbeat
                .start(transport)
                .map_err(tracerr::map_from_and_wrap!())?;
        }

        Ok(())
    }
}

impl RpcClient for WebSocketRpcClient {
    /// Creates new WebSocket connection to remote media server.
    /// Starts `Heartbeat` if connection succeeds and binds handlers
    /// on receiving messages from a server and closing socket.
    fn connect(
        &self,
        transport: Rc<dyn RpcTransport>,
    ) -> LocalBoxFuture<'static, Result<(), Traced<RpcClientError>>> {
        let this = Self(Rc::clone(&self.0));
        Box::pin(async move {
            this.0
                .borrow_mut()
                .heartbeat
                .start(Rc::clone(&transport))
                .map_err(tracerr::map_from_and_wrap!())?;

            let this_clone = this.downgrade();
            let mut on_socket_message = transport
                .on_message()
                .map_err(tracerr::map_from_and_wrap!())?;
            spawn_local(async move {
                while let Some(msg) = on_socket_message.next().await {
                    if let Some(this) = this_clone.upgrade() {
                        this.on_transport_message(msg)
                    }
                }
            });

            let mut on_idle = this.0.borrow_mut().heartbeat.on_idle();
            let weak_this = this.downgrade();
            spawn_local(async move {
                while let Some(_) = on_idle.next().await {
                    if let Some(this) = weak_this.upgrade() {
                        this.send_connection_loss();
                    }
                }
            });

            let this_clone = this.downgrade();
            let mut on_socket_close = transport
                .on_close()
                .map_err(tracerr::map_from_and_wrap!())?;
            spawn_local(async move {
                while let Some(msg) = on_socket_close.next().await {
                    if let Some(this) = this_clone.upgrade() {
                        this.on_transport_close(&msg);
                    }
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

    /// Returns [`LocalBoxStream`] to which will be sent
    /// [`ReconnectionHandle`] on connection losing.
    fn on_connection_loss(&self) -> LocalBoxStream<'static, ReconnectorHandle> {
        let (tx, rx) = mpsc::unbounded();
        self.0.borrow_mut().on_connection_loss_sub = Some(tx);

        Box::pin(rx)
    }
}

/// RPC client which can reconnect.
pub trait ReconnectableRpcClient {
    /// Tries to reconnect a [`RpcClient`].
    ///
    /// If reconnection already performed on [`RpcTransport`], then
    /// this function will simply subscribe on reconnection end.
    ///
    /// If connection already opened in [`RpcTransport`], then [`Future`]
    /// will be instantly resolved.
    fn reconnect(
        &self,
    ) -> LocalBoxFuture<'static, Result<(), Traced<RpcClientError>>>;

    /// Tries to reconnect [`RpcTransport`] in a loop with growing delay until
    /// it will not be reconnected.
    ///
    /// This function will consider state of [`RpcTransport`]. If
    /// [`RpcTransport`] already reconnecting, new reconnection will not be
    /// performed in this step of loop. If already
    /// started reconnection ended with [`State::Open`] then [`Future`] will
    /// simply resolve. If already started reconnection ended with
    /// [`State::Close`] or [`State::Closing`] then next step of loop will
    /// be preformed.
    ///
    /// If [`RpcTransport`] state is already [`State::Open`] then
    /// [`Future`] will be resolved immediately after `starting_delay`.
    fn reconnect_with_backoff(
        &self,
        starting_delay: JsDuration,
        multiplier: f32,
        max_delay_ms: JsDuration,
    ) -> LocalBoxFuture<'static, Result<(), Traced<RpcClientError>>>;
}

impl ReconnectableRpcClient for WebSocketRpcClient {
    fn reconnect(
        &self,
    ) -> LocalBoxFuture<'static, Result<(), Traced<RpcClientError>>> {
        let weak_this = self.downgrade();

        Box::pin(async move {
            let this = weak_this
                .upgrade()
                .ok_or_else(|| tracerr::new!(RpcClientError::RpcClientGone))?;

            let sock = this
                .0
                .borrow()
                .sock
                .as_ref()
                .map(Rc::clone)
                .ok_or_else(|| tracerr::new!(RpcClientError::NoSocket))?;

            match sock.get_state() {
                State::Connecting => {
                    let mut state_change_stream = sock.on_state_change();
                    while let Some(state) = state_change_stream.next().await {
                        match state {
                            State::Open => {
                                return Ok(());
                            }
                            State::Closed | State::Closing => {
                                return Err(tracerr::new!(
                                    RpcClientError::ReconnectionFailed
                                ));
                            }
                            _ => (),
                        }
                    }
                }
                State::Closing | State::Closed => {
                    this.try_reconnect(&sock)
                        .await
                        .map_err(tracerr::map_from_and_wrap!())?;
                    return Ok(());
                }
                State::Open => {
                    return Ok(());
                }
            };
            Ok(())
        })
    }

    fn reconnect_with_backoff(
        &self,
        starting_delay: JsDuration,
        multiplier: f32,
        max_delay_ms: JsDuration,
    ) -> LocalBoxFuture<'static, Result<(), Traced<RpcClientError>>> {
        let weak_this = self.downgrade();
        Box::pin(async move {
            let this = weak_this
                .upgrade()
                .ok_or_else(|| tracerr::new!(RpcClientError::RpcClientGone))?;
            let mut delayer =
                BackoffDelayer::new(starting_delay, multiplier, max_delay_ms);
            let sock = this
                .0
                .borrow()
                .sock
                .as_ref()
                .cloned()
                .ok_or_else(|| tracerr::new!(RpcClientError::NoSocket))?;

            loop {
                delayer
                    .delay()
                    .await
                    .map_err(tracerr::map_from_and_wrap!())?;
                match sock.get_state() {
                    State::Open => return Ok(()),
                    State::Closing | State::Closed => {
                        if this.try_reconnect(&sock).await.is_ok() {
                            return Ok(());
                        }
                    }
                    State::Connecting => {
                        if Some(State::Open)
                            == sock.on_state_change().next().await
                        {
                            return Ok(());
                        }
                    }
                }
            }
        })
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

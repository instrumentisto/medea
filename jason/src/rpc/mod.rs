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
    Event, RpcSettings, ServerMsg,
};
use serde::Serialize;
use tracerr::Traced;
use wasm_bindgen_futures::spawn_local;
use web_sys::CloseEvent;

use crate::utils::{console_error, JsCaused, JsError};

#[cfg(not(feature = "mockable"))]
use self::{
    backoff_delayer::BackoffDelayer,
    heartbeat::{Heartbeat, HeartbeatError},
};

#[cfg(feature = "mockable")]
pub use self::{
    backoff_delayer::BackoffDelayer,
    heartbeat::{Heartbeat, HeartbeatError},
};
#[doc(inline)]
pub use self::{
    heartbeat::{IdleTimeout, PingInterval},
    reconnect_handle::ReconnectorHandle,
    websocket::{TransportError, WebSocketRpcTransport},
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

/// State of [`RpcClient`] and [`RpcTransport`].
#[derive(Clone, Debug)]
pub enum State {
    /// Socket has been created. The connection is not yet open.
    ///
    /// Reflects `CONNECTING` state from JS side [`WebSocket.readyState`].
    ///
    /// [`WebSocket.readyState`]: https://tinyurl.com/t8ovwvr
    Connecting,

    /// The connection is open and ready to communicate.
    ///
    /// Reflects `OPEN` state from JS side [`WebSocket.readyState`].
    ///
    /// [`WebSocket.readyState`]: https://tinyurl.com/t8ovwvr
    Open,

    /// The connection is in the process of closing.
    ///
    /// Reflects `CLOSING` state from JS side [`WebSocket.readyState`].
    ///
    /// [`WebSocket.readyState`]: https://tinyurl.com/t8ovwvr
    Closing,

    /// The connection is closed or couldn't be opened.
    ///
    /// Reflects `CLOSED` state from JS side [`WebSocket.readyState`].
    ///
    /// [`WebSocket.readyState`]: https://tinyurl.com/t8ovwvr
    Closed(ClosedStateReason),
}

impl State {
    /// Returns JS side number of [`WebSocket.readyState`].
    ///
    /// [`WebSocket.readyState`]: https://tinyurl.com/t8ovwvr
    pub fn id(&self) -> u8 {
        match self {
            Self::Connecting => 0,
            Self::Open => 1,
            Self::Closing => 2,
            Self::Closed(_) => 3,
        }
    }
}

/// Reason of [`State::Closed`].
#[derive(Clone, Debug)]
pub enum ClosedStateReason {
    /// Connection with server was lost.
    ConnectionLost(CloseMsg),

    /// Error while creating connection between client and server.
    ConnectionFailed(TransportError),

    /// [`State`] unexpectedly become [`State::Closed`].
    ///
    /// Considered that this [`StateCloseReason`] will be never provided.
    Unknown,

    /// Indicates that connection with server has never been established.
    NeverConnected,

    /// First received [`ServerMsg`] after [`RpcClient::connect`] is not
    /// [`ServerMsg::RpcSettings`].
    FirstServerMsgIsNotRpcSettings,
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
            3 => Self::Closed(ClosedStateReason::Unknown),
            _ => unreachable!(),
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

    /// Occurs if [`Weak`] pointer to the [`RpcClient`] can't be upgraded to
    /// [`Rc`].
    #[display(fmt = "RpcClient unexpectedly gone.")]
    RpcClientGone,

    /// Occurs if [`RpcClient::connect`] fails.
    #[display(fmt = "Connection failed. {:?}", _0)]
    ConnectionFailed(ClosedStateReason),
}

// TODO: consider using async-trait crate, it doesnt work with mockall atm
/// Client to talk with server via Client API RPC.
#[cfg_attr(feature = "mockable", mockall::automock)]
pub trait RpcClient {
    /// Tries to upgrade [`State`] of this [`RpcClient`] to [`State::Open`].
    ///
    /// This function is also used for reconnection of this [`RpcClient`].
    ///
    /// If [`RpcClient`] is closed than this function will try to establish
    /// new RPC connection.
    ///
    /// If [`RpcClient`] already connecting then this function will not perform
    /// one more connection try. It will subsribe to [`State`] changes and wait
    /// for first connection result. And based on this result - this function
    /// will be resolved.
    ///
    /// If [`RpcClient`] already connected then this function will instantly
    /// resolved.
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

    /// [`Future`] which will be resolved on normal [`RpcClient`] connection
    /// closing. This [`Future`] wouldn't be resolved on abnormal closes. On
    /// abnormal close [`RpcClient::on_connection_loss`] will be throwed.
    fn on_normal_close(
        &self,
    ) -> LocalBoxFuture<'static, Result<CloseReason, oneshot::Canceled>>;

    /// Sets reason, that will be passed to underlying transport when this
    /// client will be dropped.
    fn set_close_reason(&self, close_reason: ClientDisconnect);

    /// Returns [`Stream`] to which will be sent `()` on every connection loss.
    ///
    /// Connection loss is unexpected [`RpcTransport`] close. In case of
    /// connection loss, JS side user should select reconnection strategy with
    /// [`ReconnectHandle`] (or simply close [`Room`]).
    fn on_connection_loss(&self) -> LocalBoxStream<'static, ()>;

    /// Returns current token with which this [`RpcClient`] was connected.
    ///
    /// If token is `None` then [`RpcClient`] never was connected to a server.
    fn get_token(&self) -> Option<String>;

    /// Returns current state of this [`RpcClient`].
    fn get_state(&self) -> State;

    /// Subscibes to a [`RpcClient`] [`State`] changes.
    ///
    /// This function guarantees that two identical [`State`]s in a row wouldn't
    /// sent.
    fn on_state_change(&self) -> LocalBoxStream<'static, State>;
}

/// RPC transport between a client and server.
#[cfg_attr(feature = "mockable", mockall::automock)]
pub trait RpcTransport {
    /// Returns [`LocalBoxStream`] of all messages received by this transport.
    fn on_message(
        &self,
    ) -> Result<LocalBoxStream<'static, ServerMsg>, Traced<TransportError>>;

    /// Sets reason, that will be sent to remote server when this transport will
    /// be dropped.
    fn set_close_reason(&self, reason: ClientDisconnect);

    /// Sends a message to server.
    fn send(&self, msg: &ClientMsg) -> Result<(), Traced<TransportError>>;

    /// Subscribes to the [`State`] changes.
    ///
    /// This function guarantees that two identical [`State`] variants in a row
    /// doesn't will be sent.
    fn on_state_change(&self) -> LocalBoxStream<'static, State>;
}

/// Inner state of [`WebsocketRpcClient`].
struct Inner {
    /// [`WebSocket`] connection to remote media server.
    sock: Option<Rc<dyn RpcTransport>>,

    /// Service for connection loss detection through Ping/Pong mechanism.
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

    /// Closure which will create new [`RpcTransport`] for this [`RpcClient`]
    /// on every [`RpcClient::connect`] call.
    rpc_transport_factory: RpcTransportFactory,

    /// Token with which this [`RpcClient`] was connected.
    ///
    /// Will be `None` if this [`RpcClient`] was never connected to a sever.
    token: Option<String>,

    /// Subscibers on [`State`] changes of this [`RpcClient`].
    on_state_change_subs: Vec<mpsc::UnboundedSender<State>>,

    /// Current [`State`] of this [`RpcClient`].
    state: State,
}

/// Factory closure which creates [`RpcTransport`] for
/// [`WebSocketRpcClient::establish_connecion`] function.
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
            state: State::Closed(ClosedStateReason::NeverConnected),
        }))
    }

    /// Updates [`State`] of this [`WebSocketRpcClient`] and sends
    /// update to all subs.
    ///
    /// Guarantees that two identical [`State`]s in a row doesn't
    /// will be sent.
    ///
    /// Also, outdated [`State`] change subs will be cleaned here.
    fn update_state(&mut self, state: &State) {
        if self.state.id() != state.id() {
            self.state = state.clone();
            self.on_state_change_subs.retain(|sub| !sub.is_closed());
            self.on_state_change_subs
                .iter()
                .filter_map(|sub| sub.unbounded_send(state.clone()).err())
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

    /// Function which will be called when [`RpcClient`] connection is
    /// considered as lost.
    ///
    /// Stops [`Heartbeat`], notifies all [`RpcClient::on_connection_loss`] subs
    /// about connection loss.
    fn connection_loss(&self) {
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
    fn transport_close(&self, close_msg: &CloseMsg) {
        self.0.borrow_mut().heartbeat.stop();

        match &close_msg {
            CloseMsg::Normal(_, reason) => match reason {
                CloseByServerReason::Reconnected => (),
                CloseByServerReason::Idle => {
                    self.connection_loss();
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
                self.connection_loss();
            }
        }
    }

    /// Handles [`ServerMsg`]s from a remote server.
    fn on_transport_message(&self, msg: ServerMsg) {
        match msg {
            ServerMsg::Event(event) => {
                // TODO: filter messages by session
                self.0.borrow_mut().subs.retain(|sub| !sub.is_closed());
                self.0
                    .borrow()
                    .subs
                    .iter()
                    .filter_map(|sub| sub.unbounded_send(event.clone()).err())
                    .for_each(|e| console_error(e.to_string()));
            }
            ServerMsg::RpcSettings(settings) => {
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

    /// Starts [`Heartbeat`] with provided [`RpcSettings`] for provided
    /// [`RpcTransport`].
    async fn start_heartbeat(
        &self,
        transport: Rc<dyn RpcTransport>,
        rpc_settings: RpcSettings,
    ) -> Result<(), Traced<RpcClientError>> {
        let idle_timeout = IdleTimeout(
            Duration::from_millis(rpc_settings.idle_timeout_ms).into(),
        );
        let ping_interval = PingInterval(
            Duration::from_millis(rpc_settings.ping_interval_ms).into(),
        );
        self.0
            .borrow_mut()
            .heartbeat
            .start(idle_timeout, ping_interval, transport)
            .map_err(tracerr::map_from_and_wrap!())?;

        let mut on_idle = self.0.borrow().heartbeat.on_idle();
        let weak_this = Rc::downgrade(&self.0);
        spawn_local(async move {
            while let Some(_) = on_idle.next().await {
                if let Some(this) = weak_this.upgrade().map(Self) {
                    this.connection_loss();
                }
            }
        });

        Ok(())
    }

    /// Tries to establish [`RpcClient`] connection.
    async fn establish_connection(
        &self,
        token: String,
    ) -> Result<(), Traced<RpcClientError>> {
        self.0.borrow_mut().token = Some(token.clone());
        self.0.borrow_mut().update_state(&State::Connecting);
        let create_transport_fut =
            (self.0.borrow().rpc_transport_factory)(token);
        let transport = create_transport_fut.await.map_err(|e| {
            let transport_err = e.into_inner();
            self.0.borrow_mut().update_state(&State::Closed(
                ClosedStateReason::ConnectionFailed(transport_err.clone()),
            ));
            tracerr::new!(RpcClientError::from(
                ClosedStateReason::ConnectionFailed(transport_err)
            ))
        })?;

        if let Some(msg) = transport
            .on_message()
            .map_err(tracerr::map_from_and_wrap!())?
            .next()
            .await
        {
            if let ServerMsg::RpcSettings(rpc_settings) = msg {
                self.start_heartbeat(Rc::clone(&transport), rpc_settings)
                    .await?;
                self.0.borrow_mut().update_state(&State::Open);
            } else {
                let close_reason =
                    ClosedStateReason::FirstServerMsgIsNotRpcSettings;
                self.0
                    .borrow_mut()
                    .update_state(&State::Closed(close_reason.clone()));
                return Err(tracerr::new!(RpcClientError::ConnectionFailed(
                    close_reason
                )));
            }
        } else {
            return Err(tracerr::new!(RpcClientError::NoSocket));
        }

        let mut transport_on_state_change_stream = transport.on_state_change();
        let weak_inner = Rc::downgrade(&self.0);
        spawn_local(async move {
            while let Some(state) =
                transport_on_state_change_stream.next().await
            {
                if let Some(inner) = weak_inner.upgrade() {
                    let this = Self(inner);
                    if let State::Closed(reason) = &state {
                        if let ClosedStateReason::ConnectionLost(msg) = reason {
                            this.transport_close(&msg);
                        }
                    }
                    this.0.borrow_mut().update_state(&state);
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

        self.0.borrow_mut().sock.replace(transport);
        Ok(())
    }

    /// Subscribes to [`RpcClient`] [`State`] changes and when
    /// [`State::Connecting`] will be changed to something else, then this
    /// [`Future`] will be resolved and based on new [`State`] [`Result`]
    /// will be returned.
    async fn connecting_result(&self) -> Result<(), Traced<RpcClientError>> {
        let mut transport_state_stream = self.on_state_change();
        while let Some(state) = transport_state_stream.next().await {
            match state {
                State::Open => {
                    return Ok(());
                }
                State::Closed(reason) => {
                    return Err(tracerr::new!(
                        RpcClientError::ConnectionFailed(reason)
                    ));
                }
                State::Connecting | State::Closing => (),
            }
        }
        Err(tracerr::new!(RpcClientError::RpcClientGone))
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
    fn connect(
        &self,
        token: String,
    ) -> LocalBoxFuture<'static, Result<(), Traced<RpcClientError>>> {
        let this = Self(Rc::clone(&self.0));
        Box::pin(async move {
            let current_token = this.0.borrow().token.clone();
            if let Some(current_token) = current_token {
                if current_token == token {
                    match this.get_state() {
                        State::Open => Ok(()),
                        State::Connecting => this.connecting_result().await,
                        State::Closed(_) | State::Closing => {
                            this.establish_connection(token).await
                        }
                    }
                } else {
                    this.establish_connection(token).await
                }
            } else {
                this.establish_connection(token).await
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
    fn on_normal_close(
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
        self.0.borrow().state.clone()
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

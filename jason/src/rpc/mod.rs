//! Abstraction over RPC transport.

mod backoff_delayer;
mod heartbeat;
mod reconnect_handle;
pub mod websocket;

use std::{cell::RefCell, rc::Rc, time::Duration};

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
use medea_reactive::ObservableCell;
use serde::Serialize;
use tracerr::Traced;
use wasm_bindgen_futures::spawn_local;
use web_sys::CloseEvent;

use crate::utils::{console_error, JasonError, JsCaused, JsError};

use websocket::TransportState;

#[doc(inline)]
pub use self::{
    backoff_delayer::BackoffDelayer,
    heartbeat::{Heartbeat, HeartbeatError, IdleTimeout, PingInterval},
    reconnect_handle::ReconnectHandle,
    websocket::{RpcTransport, TransportError, WebSocketRpcTransport},
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
    ///
    /// [`Room`]: crate::api::Room
    RoomUnexpectedlyDropped,

    /// [`Room`] was normally closed by JS side.
    ///
    /// [`Room`]: crate::api::Room
    RoomClosed,

    /// [`RpcClient`] was unexpectedly dropped.
    ///
    /// [`RpcClient`]: crate::rpc::RpcClient
    RpcClientUnexpectedlyDropped,

    /// [`RpcTransport`] was unexpectedly dropped.
    ///
    /// [`RpcTransport`]: crate::rpc::RpcTransport
    RpcTransportUnexpectedlyDropped,
}

impl ClientDisconnect {
    /// Returns `true` if [`ClientDisconnect`] is considered as error.
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
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CloseMsg {
    /// Transport was gracefully closed by remote.
    ///
    /// Determines by close code `1000` and existence of
    /// [`CloseByServerReason`].
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

#[derive(Clone, Debug, PartialEq, From)]
struct IsReconnected(bool);

/// State of [`RpcClient`] and [`RpcTransport`].
#[derive(Clone, Debug, PartialEq)]
enum ClientState {
    /// [`RpcClient`] is currently establishing connection to RPC server.
    Connecting,
    /// Connection with RPC Server is active.
    Open(IsReconnected),
    /// Connection with RPC server is currently closed.
    Closed(ClosedStateReason),
}

/// The reason of why [`RpcClient`]/[`RpcTransport`] went into
/// [`State::Closed`].
#[derive(Clone, Debug, PartialEq)]
pub enum ClosedStateReason {
    /// Connection with server was lost.
    ConnectionLost(CloseMsg),

    /// Error while creating connection between client and server.
    ConnectionFailed(TransportError),

    /// [`State`] unexpectedly become [`State::Closed`].
    ///
    /// Considered that this [`ClosedStateReason`] will be never provided.
    Unknown,

    /// Indicates that connection with server has never been established.
    NeverConnected,

    /// First received [`ServerMsg`] after [`RpcClient::connect`] is not
    /// [`ServerMsg::RpcSettings`].
    FirstServerMsgIsNotRpcSettings,

    /// Connection has been inactive for a while and thus considered idle
    /// by a client.
    Idle,
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
    ///
    /// [`Weak`]: std::rc::Weak
    #[display(fmt = "RpcClient unexpectedly gone.")]
    RpcClientGone,

    /// Occurs if [`RpcClient::connect`] fails.
    #[display(fmt = "Connection failed. {:?}", _0)]
    ConnectionFailed(ClosedStateReason),
}

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
    /// If [`RpcClient`] already in [`State::Connecting`] then this function
    /// will not perform one more connection try. It will subsribe to
    /// [`State`] changes and wait for first connection result. And based on
    /// this result - this function will be resolved.
    ///
    /// If [`RpcClient`] already in [`State::Open`] then this function will be
    /// instantly resolved.
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

    /// [`Future`] which will resolve on normal [`RpcClient`] connection
    /// closing.
    ///
    /// This [`Future`] wouldn't be resolved on abnormal closes. On
    /// abnormal close [`RpcClient::on_connection_loss`] will be thrown.
    ///
    /// [`Future`]: std::future::Future
    fn on_normal_close(
        &self,
    ) -> LocalBoxFuture<'static, Result<CloseReason, oneshot::Canceled>>;

    /// Sets reason, that will be passed to underlying transport when this
    /// client will be dropped.
    fn set_close_reason(&self, close_reason: ClientDisconnect);

    /// Subscribe to connection loss events.
    ///
    /// Connection loss is any unexpected [`RpcTransport`] close. In case of
    /// connection loss, JS side user should select reconnection strategy with
    /// [`ReconnectHandle`] (or simply close [`Room`]).
    ///
    /// [`Room`]: crate::api::Room
    /// [`Stream`]: futures::Stream
    fn on_connection_loss(&self) -> LocalBoxStream<'static, ()>;

    /// Subscribe to reconnected events.
    ///
    /// This will fire when connection to RPC server is reestablished after
    /// connection loss.
    fn on_reconnected(&self) -> LocalBoxStream<'static, ()>;

    /// Returns current token with which this [`RpcClient`] was connected.
    ///
    /// If token is `None` then [`RpcClient`] never was connected to a server.
    fn get_token(&self) -> Option<String>;
}

/// Inner state of [`WebsocketRpcClient`].
struct Inner {
    /// [`WebSocket`] connection to remote media server.
    sock: Option<Rc<dyn RpcTransport>>,

    /// Connection loss detector via ping/pong mechanism.
    heartbeat: Option<Heartbeat>,

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

    /// Senders for [`RpcClient::on_connection_loss`] subscribers.
    on_connection_loss_subs: Vec<mpsc::UnboundedSender<()>>,

    /// Closure which will create new [`RpcTransport`]s for this [`RpcClient`]
    /// on every [`WebSocketRpcClient::establish_connection`] call.
    rpc_transport_factory: RpcTransportFactory,

    /// Token with which this [`RpcClient`] was connected.
    ///
    /// Will be `None` if this [`RpcClient`] was never connected to a sever.
    token: Option<String>,

    /// Current [`State`] of this [`RpcClient`].
    state: ObservableCell<ClientState>,
}

/// Factory closure which creates [`RpcTransport`] for
/// [`WebSocketRpcClient::establish_connection`] function.
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
            subs: Vec::new(),
            heartbeat: None,
            close_reason: ClientDisconnect::RpcClientUnexpectedlyDropped,
            on_connection_loss_subs: Vec::new(),
            rpc_transport_factory,
            token: None,
            state: ObservableCell::new(ClientState::Closed(
                ClosedStateReason::NeverConnected,
            )),
        }))
    }
}

// TODO:
// 1. Proper sub registry.
// 2. Reconnect.
// 3. Disconnect if no pongs.
// 4. Buffering if no socket?
/// Client API RPC client to talk with server via [WebSocket].
///
/// [WebSocket]: https://developer.mozilla.org/ru/docs/WebSockets
pub struct WebSocketRpcClient(Rc<RefCell<Inner>>);

impl WebSocketRpcClient {
    /// Creates new [`WebSocketRpcClient`] with provided [`RpcTransportFactory`]
    /// closure.
    pub fn new(rpc_transport_factory: RpcTransportFactory) -> Self {
        Self(Inner::new(rpc_transport_factory))
    }

    /// Stops [`Heartbeat`] and notifies all [`RpcClient::on_connection_loss`]
    /// subs about connection loss.
    fn handle_connection_loss(&self, closed_state_reason: ClosedStateReason) {
        self.0
            .borrow()
            .state
            .set(ClientState::Closed(closed_state_reason));
        self.0.borrow_mut().heartbeat.take();
        self.0
            .borrow_mut()
            .on_connection_loss_subs
            .retain(|sub| !sub.is_closed());

        for sub in &self.0.borrow().on_connection_loss_subs {
            let _ = sub.unbounded_send(());
        }
    }

    /// Handles [`CloseMsg`] from a remote server.
    ///
    /// This function will be called on every WebSocket close (normal and
    /// abnormal) regardless of the [`CloseReason`].
    fn handle_close_message(&self, close_msg: CloseMsg) {
        self.0.borrow_mut().heartbeat.take();

        match close_msg {
            CloseMsg::Normal(_, reason) => match reason {
                CloseByServerReason::Reconnected => (),
                CloseByServerReason::Idle => {
                    self.handle_connection_loss(
                        ClosedStateReason::ConnectionLost(close_msg),
                    );
                }
                _ => {
                    self.0.borrow_mut().sock.take();
                    self.0
                        .borrow_mut()
                        .on_close_subscribers
                        .drain(..)
                        .filter_map(|sub| {
                            sub.send(CloseReason::ByServer(reason)).err()
                        })
                        .for_each(|reason| {
                            console_error(format!(
                                "Failed to send reason of Jason close to \
                                 subscriber: {:?}",
                                reason
                            ))
                        });
                }
            },
            CloseMsg::Abnormal(_) => {
                self.handle_connection_loss(ClosedStateReason::ConnectionLost(
                    close_msg,
                ));
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
                )
                .map_err(tracerr::wrap!(=> RpcClientError))
                .map_err(JasonError::from)
                .map_err(console_error)
                .ok();
            }
            ServerMsg::Ping(_) => {}
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

        let heartbeat =
            Heartbeat::start(transport, ping_interval, idle_timeout);

        let mut on_idle = heartbeat.on_idle();
        let weak_this = Rc::downgrade(&self.0);
        spawn_local(async move {
            while on_idle.next().await.is_some() {
                if let Some(this) = weak_this.upgrade().map(Self) {
                    this.handle_connection_loss(ClosedStateReason::Idle);
                }
            }
        });
        self.0.borrow_mut().heartbeat = Some(heartbeat);

        Ok(())
    }

    /// Tries to establish [`RpcClient`] connection.
    async fn establish_connection(
        &self,
        token: String,
    ) -> Result<(), Traced<RpcClientError>> {
        self.0.borrow_mut().token = Some(token.clone());
        self.0.borrow().state.set(ClientState::Connecting);

        // wait for transport open
        let create_transport_fut =
            (self.0.borrow().rpc_transport_factory)(token);
        let transport = create_transport_fut.await.map_err(|e| {
            let transport_err = e.into_inner();
            self.0.borrow().state.set(ClientState::Closed(
                ClosedStateReason::ConnectionFailed(transport_err.clone()),
            ));
            tracerr::new!(RpcClientError::from(
                ClosedStateReason::ConnectionFailed(transport_err)
            ))
        })?;

        // wait for ServerMsg::RpcSettings
        if let Some(msg) = transport.on_message().next().await {
            if let ServerMsg::RpcSettings(rpc_settings) = msg {
                self.start_heartbeat(Rc::clone(&transport), rpc_settings)
                    .await?;
            } else {
                let close_reason =
                    ClosedStateReason::FirstServerMsgIsNotRpcSettings;
                self.0
                    .borrow()
                    .state
                    .set(ClientState::Closed(close_reason.clone()));
                return Err(tracerr::new!(RpcClientError::ConnectionFailed(
                    close_reason
                )));
            }
        } else {
            self.0.borrow().state.set(ClientState::Closed(
                ClosedStateReason::FirstServerMsgIsNotRpcSettings,
            ));
            return Err(tracerr::new!(RpcClientError::NoSocket));
        }

        // subscribe to transport close
        let mut transport_state_changes = transport.on_state_change();
        let weak_inner = Rc::downgrade(&self.0);
        spawn_local(async move {
            while let Some(state) = transport_state_changes.next().await {
                if let Some(this) = weak_inner.upgrade().map(Self) {
                    if let TransportState::Closed(msg) = state {
                        this.handle_close_message(msg);
                    }
                }
            }
        });

        // subscribe to transport message received
        let this_clone = Rc::downgrade(&self.0);
        let mut on_socket_message = transport.on_message();
        spawn_local(async move {
            while let Some(msg) = on_socket_message.next().await {
                if let Some(this) = this_clone.upgrade().map(Self) {
                    this.on_transport_message(msg)
                }
            }
        });

        let is_reconnected =
            self.0.borrow_mut().sock.replace(transport).is_some();
        self.0
            .borrow()
            .state
            .set(ClientState::Open(is_reconnected.into()));

        Ok(())
    }

    /// Subscribes to [`RpcClient`]'s [`State`] changes and when
    /// [`State::Connecting`] will be changed to something else, then this
    /// [`Future`] will be resolved and based on new [`State`] [`Result`]
    /// will be returned.
    async fn connecting_result(&self) -> Result<(), Traced<RpcClientError>> {
        let mut state_changes = self.0.borrow().state.subscribe();
        while let Some(state) = state_changes.next().await {
            match state {
                ClientState::Open(_) => {
                    return Ok(());
                }
                ClientState::Closed(reason) => {
                    return Err(tracerr::new!(
                        RpcClientError::ConnectionFailed(reason)
                    ));
                }
                ClientState::Connecting => (),
            }
        }
        Err(tracerr::new!(RpcClientError::RpcClientGone))
    }

    /// Updates RPC settings of this [`RpcClient`].
    fn update_settings(
        &self,
        idle_timeout: IdleTimeout,
        ping_interval: PingInterval,
    ) -> Result<(), Traced<RpcClientError>> {
        self.0
            .borrow_mut()
            .heartbeat
            .as_ref()
            .ok_or_else(|| tracerr::new!(RpcClientError::NoSocket))
            .map(|heartbeat| {
                heartbeat.update_settings(idle_timeout, ping_interval)
            })
    }
}

impl RpcClient for WebSocketRpcClient {
    fn connect(
        &self,
        token: String,
    ) -> LocalBoxFuture<'static, Result<(), Traced<RpcClientError>>> {
        let weak_inner = Rc::downgrade(&self.0);
        Box::pin(async move {
            if let Some(this) = weak_inner.upgrade().map(Self) {
                let current_token = this.0.borrow().token.clone();
                if let Some(current_token) = current_token {
                    if current_token == token {
                        let state = this.0.borrow().state.borrow().clone();
                        match state {
                            ClientState::Open(_) => Ok(()),
                            ClientState::Connecting => {
                                this.connecting_result().await
                            }
                            ClientState::Closed(_) => {
                                this.establish_connection(token).await
                            }
                        }
                    } else {
                        this.establish_connection(token).await
                    }
                } else {
                    this.establish_connection(token).await
                }
            } else {
                Err(tracerr::new!(RpcClientError::NoSocket))
            }
        })
    }

    // TODO: proper sub registry
    fn subscribe(&self) -> LocalBoxStream<'static, Event> {
        let (tx, rx) = mpsc::unbounded();
        self.0.borrow_mut().subs.push(tx);

        Box::pin(rx)
    }

    // TODO: proper sub registry
    fn unsub(&self) {
        self.0.borrow_mut().subs.clear();
    }

    // TODO: proper sub registry
    fn send_command(&self, command: Command) {
        let socket_borrow = &self.0.borrow().sock;

        // TODO: no socket? we dont really want this method to return err
        if let Some(socket) = socket_borrow.as_ref() {
            if let Err(err) = socket.send(&ClientMsg::Command(command)) {
                // TODO: we will just wait for reconnect at this moment
                //       should be handled properly as a part of future
                //       state synchronization mechanism
                //       PR: https://github.com/instrumentisto/medea/pull/51
                JasonError::from(err).print()
            }
        }
    }

    fn on_normal_close(
        &self,
    ) -> LocalBoxFuture<'static, Result<CloseReason, oneshot::Canceled>> {
        let (tx, rx) = oneshot::channel();
        self.0.borrow_mut().on_close_subscribers.push(tx);
        Box::pin(rx)
    }

    fn on_connection_loss(&self) -> LocalBoxStream<'static, ()> {
        let (tx, rx) = mpsc::unbounded();
        self.0.borrow_mut().on_connection_loss_subs.push(tx);
        Box::pin(rx)
    }

    fn on_reconnected(&self) -> LocalBoxStream<'static, ()> {
        self.0
            .borrow()
            .state
            .subscribe()
            .filter_map(|state| async move {
                if state == ClientState::Open(IsReconnected(true)) {
                    Some(())
                } else {
                    None
                }
            })
            .boxed_local()
    }

    fn set_close_reason(&self, close_reason: ClientDisconnect) {
        self.0.borrow_mut().close_reason = close_reason
    }

    fn get_token(&self) -> Option<String> {
        self.0.borrow().token.clone()
    }
}

impl Drop for Inner {
    /// Drops related connection and its [`Heartbeat`].
    fn drop(&mut self) {
        if let Some(socket) = self.sock.take() {
            socket.set_close_reason(self.close_reason);
        }
    }
}

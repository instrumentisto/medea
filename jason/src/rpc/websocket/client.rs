use std::{cell::RefCell, rc::Rc, time::Duration};

use derive_more::Display;
use futures::{
    channel::{mpsc, oneshot},
    future::LocalBoxFuture,
    stream::{LocalBoxStream, StreamExt as _},
};
use medea_client_api_proto::{
    ClientMsg, CloseReason as CloseByServerReason, Command, Credential, Event,
    MemberId, RoomId, RpcSettings, ServerMsg,
};
use medea_reactive::ObservableCell;
use serde::Serialize;
use tracerr::Traced;
use wasm_bindgen_futures::spawn_local;

use crate::{
    rpc::{
        websocket::transport::{RpcTransport, TransportError, TransportState},
        ApiUrl, CloseMsg, CloseReason, ClosedStateReason, Heartbeat,
        IdleTimeout, PingInterval, RpcClientError,
    },
    utils::JasonError,
};

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

/// State of [`RpcClient`] and [`RpcTransport`].
#[derive(Clone, Debug, PartialEq)]
enum ClientState {
    /// [`RpcClient`] is currently establishing connection to RPC server.
    Connecting,
    /// Connection with RPC Server is active.
    Open,
    /// Connection with RPC server is currently closed.
    Closed(ClosedStateReason),
}

/// Inner state of [`WebsocketRpcClient`].
struct Inner {
    /// [`WebSocket`] connection to remote media server.
    sock: Option<Rc<dyn RpcTransport>>,

    /// Connection loss detector via ping/pong mechanism.
    heartbeat: Option<Heartbeat>,

    /// Event's subscribers list.
    subs: Vec<mpsc::UnboundedSender<RpcEvent>>,

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

    /// URL to which [`RpcTransport`] will be connect.
    ///
    /// Will be `None` if this [`RpcClient`] was never connected to a sever.
    url: Option<ApiUrl>,

    /// Current [`State`] of this [`RpcClient`].
    state: ObservableCell<ClientState>,
}

/// Factory closure which creates [`RpcTransport`] for
/// [`WebSocketRpcClient::establish_connection`] function.
pub type RpcTransportFactory = Box<
    dyn Fn(
        ApiUrl,
    ) -> LocalBoxFuture<
        'static,
        Result<Rc<dyn RpcTransport>, Traced<TransportError>>,
    >,
>;

impl Inner {
    fn new(rpc_transport_factory: RpcTransportFactory) -> RefCell<Self> {
        RefCell::new(Self {
            sock: None,
            on_close_subscribers: Vec::new(),
            subs: Vec::new(),
            heartbeat: None,
            close_reason: ClientDisconnect::RpcClientUnexpectedlyDropped,
            on_connection_loss_subs: Vec::new(),
            rpc_transport_factory,
            url: None,
            state: ObservableCell::new(ClientState::Closed(
                ClosedStateReason::NeverConnected,
            )),
        })
    }
}

/// Events which can be thrown by [`WebSocketRpcClient`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RpcEvent {
    /// Notification of the subscribers that [`WebSocketRpcClient`] is joined
    /// `Room` on the Media Server.
    JoinedRoom {
        /// [`RoomId`] of the joined `Room`.
        room_id: RoomId,

        /// [`MemberId`] of the joined `Member`.
        member_id: MemberId,
    },

    /// Notification of the subscribers that [`WebSocketRpcClient`] leaved
    /// `Room` on the Media Server.
    LeftRoom {
        /// [`RoomId`] of the leaved `Room`.
        room_id: RoomId,

        /// Reason of the `Room` leaving.
        close_reason: CloseReason,
    },

    /// [`WebSocketRpcClient`] received [`Event`] from the Media Server.
    Event {
        /// [`RoomId`] of the `Room` for which this [`Event`] was received.
        room_id: RoomId,

        /// Received [`Event`].
        event: Event,
    },
}

/// Client API RPC client to talk with server via [WebSocket].
///
/// [WebSocket]: https://developer.mozilla.org/ru/docs/WebSockets
pub struct WebSocketRpcClient(RefCell<Inner>);

impl WebSocketRpcClient {
    /// Creates new [`WebSocketRpcClient`] with provided [`RpcTransportFactory`]
    /// closure.
    pub fn new(rpc_transport_factory: RpcTransportFactory) -> Self {
        Self(Inner::new(rpc_transport_factory))
    }

    /// Sends [`ClientMsg`] to the Media Server.
    ///
    /// If some error occurs while sending message, then it will be printed with
    /// [`JasonError::print`].
    fn send_msg(&self, msg: &ClientMsg) {
        let inner = self.0.borrow();
        if let Some(sock) = &inner.sock {
            if let Err(e) = sock
                .send(msg)
                .map_err(tracerr::map_from_and_wrap!(=> TransportError))
            {
                JasonError::from(e).print();
            }
        }
    }

    /// Authorizes [`WebSocketRpcClient`] on the Media Server.
    pub fn authorize(
        &self,
        room_id: RoomId,
        member_id: MemberId,
        credential: Credential,
    ) {
        self.send_msg(&ClientMsg::JoinRoom {
            room_id,
            member_id,
            credential,
        });
    }

    /// Leaves `Room` with a provided [`RoomId`].
    pub fn leave_room(&self, room_id: RoomId, member_id: MemberId) {
        self.send_msg(&ClientMsg::LeaveRoom { room_id, member_id });
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
            .retain(|sub| sub.unbounded_send(()).is_ok());
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
                        .for_each(|sub| {
                            let _ = sub.send(CloseReason::ByServer(reason));
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
        let msg = match msg {
            ServerMsg::Event { room_id, event } => {
                Some(RpcEvent::Event { room_id, event })
            }
            ServerMsg::RpcSettings(settings) => {
                self.update_settings(
                    IdleTimeout(
                        Duration::from_millis(settings.idle_timeout_ms.into())
                            .into(),
                    ),
                    PingInterval(
                        Duration::from_millis(settings.ping_interval_ms.into())
                            .into(),
                    ),
                )
                .map_err(tracerr::wrap!(=> RpcClientError))
                .map_err(|e| {
                    log::error!("Failed to update socket settings: {}", e)
                })
                .ok();
                None
            }
            ServerMsg::Ping(_) => None,
            ServerMsg::JoinedRoom { room_id, member_id } => {
                Some(RpcEvent::JoinedRoom { room_id, member_id })
            }
            ServerMsg::LeftRoom {
                room_id,
                close_reason,
            } => Some(RpcEvent::LeftRoom {
                room_id,
                close_reason: CloseReason::ByServer(close_reason),
            }),
        };
        if let Some(msg) = msg {
            self.0
                .borrow_mut()
                .subs
                .retain(|sub| sub.unbounded_send(msg.clone()).is_ok());
        }
    }

    /// Starts [`Heartbeat`] with provided [`RpcSettings`] for provided
    /// [`RpcTransport`].
    async fn start_heartbeat(
        self: Rc<Self>,
        transport: Rc<dyn RpcTransport>,
        rpc_settings: RpcSettings,
    ) -> Result<(), Traced<RpcClientError>> {
        let idle_timeout = IdleTimeout(
            Duration::from_millis(rpc_settings.idle_timeout_ms.into()).into(),
        );
        let ping_interval = PingInterval(
            Duration::from_millis(rpc_settings.ping_interval_ms.into()).into(),
        );

        let heartbeat =
            Heartbeat::start(transport, ping_interval, idle_timeout);

        let mut on_idle = heartbeat.on_idle();
        let weak_this = Rc::downgrade(&self);
        spawn_local(async move {
            while on_idle.next().await.is_some() {
                if let Some(this) = weak_this.upgrade() {
                    this.handle_connection_loss(ClosedStateReason::Idle);
                }
            }
        });
        self.0.borrow_mut().heartbeat = Some(heartbeat);

        Ok(())
    }

    /// Tries to establish [`RpcClient`] connection.
    async fn establish_connection(
        self: Rc<Self>,
        url: ApiUrl,
    ) -> Result<(), Traced<RpcClientError>> {
        self.0.borrow_mut().url = Some(url.clone());
        self.0.borrow().state.set(ClientState::Connecting);

        // wait for transport open
        let create_transport_fut = (self.0.borrow().rpc_transport_factory)(url);
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
                Rc::clone(&self)
                    .start_heartbeat(Rc::clone(&transport), rpc_settings)
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
        let weak_this = Rc::downgrade(&self);
        spawn_local(async move {
            while let Some(state) = transport_state_changes.next().await {
                if let Some(this) = weak_this.upgrade() {
                    if let TransportState::Closed(msg) = state {
                        this.handle_close_message(msg);
                    }
                }
            }
        });

        // subscribe to transport message received
        let weak_this = Rc::downgrade(&self);
        let mut on_socket_message = transport.on_message();
        spawn_local(async move {
            while let Some(msg) = on_socket_message.next().await {
                if let Some(this) = weak_this.upgrade() {
                    this.on_transport_message(msg)
                }
            }
        });

        self.0.borrow_mut().sock.replace(transport);
        self.0.borrow().state.set(ClientState::Open);

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
                ClientState::Open => {
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
    ///
    /// # Errors
    ///
    /// Errors if [`WebSocketRpcClient::establish_connection`] fails.
    pub async fn connect(
        self: Rc<Self>,
        url: ApiUrl,
    ) -> Result<(), Traced<RpcClientError>> {
        let current_url = self.0.borrow().url.clone();
        if let Some(current_url) = current_url {
            if current_url == url {
                let state = self.0.borrow().state.borrow().clone();
                match state {
                    ClientState::Open => Ok(()),
                    ClientState::Connecting => self.connecting_result().await,
                    ClientState::Closed(_) => {
                        self.establish_connection(url).await
                    }
                }
            } else {
                self.establish_connection(url).await
            }
        } else {
            self.establish_connection(url).await
        }
    }

    /// Subscribes on this [`WebSocketRpcClient`]'s [`RpcEvent`]s.
    pub fn subscribe(&self) -> LocalBoxStream<'static, RpcEvent> {
        let (tx, rx) = mpsc::unbounded();
        self.0.borrow_mut().subs.push(tx);

        Box::pin(rx)
    }

    /// Sends [`Command`] for the provided [`RoomId`] to server.
    pub fn send_command(&self, room_id: RoomId, command: Command) {
        let socket_borrow = &self.0.borrow().sock;

        // TODO: no socket? we dont really want this method to return err
        if let Some(socket) = socket_borrow.as_ref() {
            if let Err(err) =
                socket.send(&ClientMsg::Command { room_id, command })
            {
                // TODO: we will just wait for reconnect at this moment
                //       should be handled properly as a part of future
                //       state synchronization mechanism
                //       PR: https://github.com/instrumentisto/medea/pull/51
                JasonError::from(err).print()
            }
        }
    }

    /// [`Future`] which will resolve on normal [`WebSocketRpcClient`]
    /// connection closing.
    ///
    /// This [`Future`] wouldn't be resolved on abnormal closes. On
    /// abnormal close [`RpcClient::on_connection_loss`] will be thrown.
    ///
    /// [`Future`]: std::future::Future
    pub fn on_normal_close(
        &self,
    ) -> LocalBoxFuture<'static, Result<CloseReason, oneshot::Canceled>> {
        let (tx, rx) = oneshot::channel();
        self.0.borrow_mut().on_close_subscribers.push(tx);
        Box::pin(rx)
    }

    /// Subscribe to connection loss events.
    ///
    /// Connection loss is any unexpected [`RpcTransport`] close. In case of
    /// connection loss, JS side user should select reconnection strategy with
    /// [`ReconnectHandle`] (or simply close [`Room`]).
    ///
    /// [`Room`]: crate::api::Room
    /// [`Stream`]: futures::Stream
    pub fn on_connection_loss(&self) -> LocalBoxStream<'static, ()> {
        let (tx, rx) = mpsc::unbounded();
        self.0.borrow_mut().on_connection_loss_subs.push(tx);
        Box::pin(rx)
    }

    /// Sets reason, that will be passed to underlying transport when this
    /// client will be dropped.
    pub fn set_close_reason(&self, close_reason: ClientDisconnect) {
        self.0.borrow_mut().close_reason = close_reason
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

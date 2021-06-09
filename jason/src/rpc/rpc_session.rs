//! Wrapper around [WebSocket] based transport that implements `Room`
//! management.
//!
//! [WebSocket]: https://developer.mozilla.org/ru/docs/WebSockets

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use async_trait::async_trait;
use derivative::Derivative;
use derive_more::{Display, From};
use futures::{
    channel::mpsc,
    future::{self, LocalBoxFuture},
    stream::LocalBoxStream,
    StreamExt,
};
use medea_client_api_proto::{Command, Event, MemberId, RoomId};
use medea_reactive::ObservableCell;
use tracerr::Traced;

use crate::{
    platform,
    rpc::{
        websocket::RpcEventHandler, ClientDisconnect, CloseReason,
        ConnectionInfo, RpcClientError, WebSocketRpcClient,
    },
    utils::JsCaused,
};

/// Errors which are can be returned from the [`WebSocketRpcSession`].
#[derive(Clone, Debug, From, JsCaused, Display)]
#[js(error = "platform::Error")]
pub enum SessionError {
    /// [`WebSocketRpcSession`] goes into [`SessionState::Finished`] and can't
    /// be used.
    #[display(fmt = "RPC Session finished with {:?} close reason", _0)]
    SessionFinished(CloseReason),

    /// [`WebSocketRpcSession`] doesn't have any credentials to authorize with.
    #[display(
        fmt = "RPC Session doesn't have any credentials to authorize with"
    )]
    NoCredentials,

    /// [`WebSocketRpcSession`] authorization on the server was failed.
    #[display(fmt = "Failed to authorize RPC session")]
    AuthorizationFailed,

    /// [`WebSocketRpcClient`] returned [`RpcClientError`].
    #[display(fmt = "RpcClientError: {:?}", _0)]
    RpcClient(#[js(cause)] RpcClientError),

    /// [`WebSocketRpcSession`] was unexpectedly dropped.
    #[display(fmt = "RPC Session was unexpectedly dropped")]
    SessionUnexpectedlyDropped,

    /// [`WebSocketRpcClient`] lost connection with a server.
    #[display(fmt = "Connection with a server was lost")]
    ConnectionLost,

    /// [`WebSocketRpcSession::connect`] called while connecting to the server.
    ///
    /// So old connection process was canceled.
    #[display(fmt = "New connection info was provided")]
    NewConnectionInfo,
}

/// Client to talk with server via Client API RPC.
#[async_trait(?Send)]
#[cfg_attr(feature = "mockable", mockall::automock)]
pub trait RpcSession {
    /// Tries to upgrade [`SessionState`] of this [`RpcSession`] to
    /// [`SessionState::Opened`].
    ///
    /// This function is also used for reconnection of this [`RpcSession`].
    ///
    /// If [`RpcSession`] is closed than this function will try to establish
    /// new RPC connection.
    ///
    /// If [`RpcSession`] already in [`SessionState::Connecting`] then this
    /// function will not perform one more connection try. It will subscribe
    /// to [`SessionState`] changes and wait for first connection result.
    /// And based on this result - this function will be resolved.
    ///
    /// If [`RpcSession`] already in [`SessionState::Opened`] then this function
    /// will be instantly resolved.
    async fn connect(
        self: Rc<Self>,
        connection_info: ConnectionInfo,
    ) -> Result<(), Traced<SessionError>>;

    /// Tries to reconnect (or connect) this [`RpcSession`] to the Media Server.
    async fn reconnect(self: Rc<Self>) -> Result<(), Traced<SessionError>>;

    /// Returns [`Stream`] of all [`Event`]s received by this [`RpcSession`].
    ///
    /// [`Stream`]: futures::Stream
    fn subscribe(&self) -> LocalBoxStream<'static, Event>;

    /// Sends [`Command`] to server.
    fn send_command(&self, command: Command);

    /// [`Future`] which will resolve on normal [`RpcSession`] connection
    /// closing.
    ///
    /// This [`Future`] wouldn't be resolved on abnormal closes. On
    /// abnormal close [`RpcSession::on_connection_loss`] will be thrown.
    ///
    /// [`Future`]: std::future::Future
    fn on_normal_close(&self) -> LocalBoxFuture<'static, CloseReason>;

    /// Sets reason, that will be passed to underlying transport when this
    /// client will be dropped.
    fn close_with_reason(&self, close_reason: ClientDisconnect);

    /// Subscribe to connection loss events.
    ///
    /// Connection loss is any unexpected [`platform::RpcTransport`] close. In
    /// case of connection loss, client side user should select reconnection
    /// strategy with [`ReconnectHandle`] (or simply close [`Room`]).
    ///
    /// [`ReconnectHandle`]: crate::rpc::ReconnectHandle
    /// [`Room`]: crate::room::Room
    fn on_connection_loss(&self) -> LocalBoxStream<'static, ()>;

    /// Subscribe to reconnected events.
    ///
    /// This will fire when connection to RPC server is reestablished after
    /// connection loss.
    fn on_reconnected(&self) -> LocalBoxStream<'static, ()>;
}

/// Client to talk with server via Client API RPC.
///
/// Responsible for [`Room`] authorization and closing.
///
/// [`Room`]: crate::room::Room
pub struct WebSocketRpcSession {
    /// [WebSocket] based Rpc Client used to talk with `Medea` server.
    ///
    /// [WebSocket]: https://developer.mozilla.org/ru/docs/WebSockets
    client: Rc<WebSocketRpcClient>,

    /// Current [`SessionState`] of this [`WebSocketRpcSession`].
    state: ObservableCell<SessionState>,

    /// Flag which indicates that [`WebSocketRpcSession`] goes to the
    /// [`SessionState::Lost`] from the [`SessionState::Opened`].
    can_reconnect: Rc<Cell<bool>>,

    /// Subscribers of the [`RpcSession::subscribe`].
    event_txs: RefCell<Vec<mpsc::UnboundedSender<Event>>>,
}

impl WebSocketRpcSession {
    /// Returns new uninitialized [`WebSocketRpcSession`] with a provided
    /// [`WebSocketRpcClient`].
    ///
    /// Spawns all [`WebSocketRpcSession`] task.
    pub fn new(client: Rc<WebSocketRpcClient>) -> Rc<Self> {
        let this = Rc::new(Self {
            client,
            state: ObservableCell::new(SessionState::Uninitialized),
            can_reconnect: Rc::new(Cell::new(false)),
            event_txs: RefCell::default(),
        });

        this.spawn_state_watcher();
        this.spawn_connection_loss_watcher();
        this.spawn_close_watcher();
        this.spawn_server_msg_listener();

        this
    }

    /// Tries to establish transport connection to media server and authorize
    /// RPC session.
    ///
    /// If [`WebSocketRpcSession`] is already trying to do it, then this method
    /// will wait for connection result and return it.
    ///
    /// # Errors
    ///
    /// Errors with [`SessionError::NoCredentials`] if current [`SessionState`]
    /// is [`SessionState::Uninitialized`].
    ///
    /// Errors with [`SessionError::SessionFinished`] if current
    /// [`SessionState`] is [`SessionState::Finished`].
    ///
    /// Errors with [`SessionError::NewConnectionInfo`] if [`SessionState`] goes
    /// into [`SessionState::Initialized`].
    ///
    /// Errors with [`SessionError::AuthorizationFailed`] if [`SessionState`]
    /// goes into [`SessionState::Uninitialized`].
    ///
    /// Errors with [`SessionError::SessionFinished`] if [`SessionState`] goes
    /// into [`SessionState::Finished`].
    ///
    /// Errors with [`SessionError`] if [`SessionState`] goes into
    /// [`SessionState::Lost`].
    async fn inner_connect(self: Rc<Self>) -> Result<(), Traced<SessionError>> {
        use SessionError as E;
        use SessionState as S;

        match self.state.get() {
            S::Connecting(_) | S::Authorizing(_) | S::Opened(_) => {}
            S::Initialized(info) | S::Lost(_, info) => {
                self.state.set(S::Connecting(info));
            }
            S::Uninitialized => {
                return Err(tracerr::new!(E::NoCredentials));
            }
            S::Finished(reason) => {
                return Err(tracerr::new!(E::SessionFinished(reason)));
            }
        }

        let mut state_updates_stream = self.state.subscribe();
        while let Some(state) = state_updates_stream.next().await {
            match state {
                S::Opened(_) => return Ok(()),
                S::Initialized(_) => {
                    return Err(tracerr::new!(E::NewConnectionInfo));
                }
                S::Lost(err, _) => {
                    // TODO: Clone Traced and add new Frame to it when Traced
                    //       cloning will be implemented.
                    return Err(tracerr::new!(AsRef::<SessionError>::as_ref(
                        &err.as_ref()
                    )
                    .clone()));
                }
                S::Uninitialized => {
                    return Err(tracerr::new!(E::AuthorizationFailed));
                }
                S::Finished(reason) => {
                    return Err(tracerr::new!(E::SessionFinished(reason)));
                }
                _ => {}
            }
        }

        Err(tracerr::new!(E::SessionUnexpectedlyDropped))
    }

    /// Spawns [`SessionState`] updates handler for this
    /// [`WebSocketRpcSession`].
    fn spawn_state_watcher(self: &Rc<Self>) {
        use SessionState as S;

        let mut state_updates = self.state.subscribe();
        let weak_this = Rc::downgrade(self);
        platform::spawn(async move {
            while let Some(state) = state_updates.next().await {
                let this = upgrade_or_break!(weak_this);
                match state {
                    S::Connecting(info) => {
                        match Rc::clone(&this.client)
                            .connect(info.url.clone())
                            .await
                            .map_err(tracerr::map_from_and_wrap!())
                        {
                            Ok(_) => {
                                this.state.set(S::Authorizing(info));
                            }
                            Err(e) => {
                                this.state.set(S::Lost(Rc::new(e), info));
                            }
                        }
                    }
                    S::Authorizing(info) => {
                        this.client.authorize(
                            info.room_id.clone(),
                            info.member_id.clone(),
                            info.credential.clone(),
                        );
                    }
                    _ => {}
                }
            }
        });
    }

    /// Spawns [`WebSocketRpcClient::on_connection_loss`] listener.
    ///
    /// Handler for the [`WebSocketRpcClient::on_connection_loss`].
    ///
    /// Sets [`WebSocketRpcSession::state`] to the [`SessionState::Lost`].
    fn spawn_connection_loss_watcher(self: &Rc<Self>) {
        use SessionState as S;

        let mut client_on_connection_loss = self.client.on_connection_loss();
        let weak_this = Rc::downgrade(self);
        platform::spawn(async move {
            while client_on_connection_loss.next().await.is_some() {
                let this = upgrade_or_break!(weak_this);

                let state = this.state.get();
                if matches!(state, S::Opened(_)) {
                    this.can_reconnect.set(true);
                }
                match state {
                    S::Connecting(info)
                    | S::Authorizing(info)
                    | S::Opened(info) => {
                        this.state.set(S::Lost(
                            Rc::new(tracerr::new!(
                                SessionError::ConnectionLost
                            )),
                            info,
                        ));
                    }
                    S::Uninitialized
                    | S::Initialized(_)
                    | S::Lost(_, _)
                    | S::Finished(_) => {}
                }
            }
        });
    }

    /// Spawns [`WebSocketRpcClient::on_normal_close`] listener.
    fn spawn_close_watcher(self: &Rc<Self>) {
        let on_normal_close = self.client.on_normal_close();
        let weak_this = Rc::downgrade(self);
        platform::spawn(async move {
            let reason = on_normal_close.await.unwrap_or_else(|_| {
                ClientDisconnect::RpcClientUnexpectedlyDropped.into()
            });
            if let Some(this) = weak_this.upgrade() {
                this.state.set(SessionState::Finished(reason));
            }
        });
    }

    /// Spawns [`WebSocketRpcClient::subscribe`] listener.
    fn spawn_server_msg_listener(self: &Rc<Self>) {
        let mut server_msg_rx = self.client.subscribe();
        let weak_this = Rc::downgrade(self);
        platform::spawn(async move {
            while let Some(msg) = server_msg_rx.next().await {
                let this = upgrade_or_break!(weak_this);
                msg.dispatch_with(this.as_ref());
            }
        })
    }
}

#[async_trait(?Send)]
impl RpcSession for WebSocketRpcSession {
    /// Tries to connect to the server with a provided [`ConnectionInfo`].
    ///
    /// # Errors
    ///
    /// Errors with [`SessionError::SessionFinished`] if current
    /// [`SessionState`] is [`SessionState::Finished`].
    ///
    /// Errors with [`SessionError`] if [`WebSocketRpcSession::connect`] errors.
    async fn connect(
        self: Rc<Self>,
        connection_info: ConnectionInfo,
    ) -> Result<(), Traced<SessionError>> {
        use SessionState as S;

        match self.state.get() {
            S::Uninitialized | S::Initialized(_) | S::Lost(_, _) => {
                self.state.set(S::Initialized(Rc::new(connection_info)));
            }
            S::Finished(reason) => {
                return Err(tracerr::new!(SessionError::SessionFinished(
                    reason
                )));
            }
            S::Connecting(info) => {
                if info.as_ref() != &connection_info {
                    self.state.set(S::Initialized(Rc::new(connection_info)));
                }
            }
            S::Authorizing(info) | S::Opened(info) => {
                if info.as_ref() != &connection_info {
                    unimplemented!(
                        "Changing `ConnectionInfo` with active or pending \
                         authorization is not supported"
                    );
                }
            }
        }

        self.inner_connect()
            .await
            .map_err(tracerr::map_from_and_wrap!())?;

        Ok(())
    }

    /// Tries to reconnect this [`WebSocketRpcSession`] to the server.
    async fn reconnect(self: Rc<Self>) -> Result<(), Traced<SessionError>> {
        self.inner_connect()
            .await
            .map_err(tracerr::map_from_and_wrap!())?;

        Ok(())
    }

    fn subscribe(&self) -> LocalBoxStream<'static, Event> {
        let (tx, rx) = mpsc::unbounded();
        self.event_txs.borrow_mut().push(tx);
        Box::pin(rx)
    }

    /// Sends [`Command`] to the server if current [`SessionState`] is
    /// [`SessionState::Opened`].
    #[inline]
    fn send_command(&self, command: Command) {
        if let SessionState::Opened(info) = self.state.get() {
            self.client.send_command(info.room_id.clone(), command);
        }
    }

    /// Returns [`Future`] which will be resolved when [`SessionState`] will be
    /// transited to the [`SessionState::Finished`] or [`WebSocketRpcSession`]
    /// will be dropped.
    ///
    /// [`Future`]: std::future::Future
    fn on_normal_close(&self) -> LocalBoxFuture<'static, CloseReason> {
        let mut state_stream = self
            .state
            .subscribe()
            .filter_map(|s| async move {
                if let SessionState::Finished(reason) = s {
                    Some(reason)
                } else {
                    None
                }
            })
            .boxed_local();
        Box::pin(async move {
            state_stream.next().await.unwrap_or_else(|| {
                ClientDisconnect::SessionUnexpectedlyDropped.into()
            })
        })
    }

    /// Closes [`WebSocketRpcSession`] with a provided [`ClientDisconnect`]
    /// reason.
    ///
    /// [`SessionState`] will be transited to the [`SessionState::Finished`].
    ///
    /// Provided [`ClientDisconnect`] will be provided to the underlying
    /// [`WebSocketRpcClient`] with [`WebSocketRpcClient::set_close_reason`].
    fn close_with_reason(&self, close_reason: ClientDisconnect) {
        if let SessionState::Opened(info) = self.state.get() {
            self.client
                .leave_room(info.room_id.clone(), info.member_id.clone());
        }

        self.client.set_close_reason(close_reason);
        self.state.set(SessionState::Finished(close_reason.into()));
    }

    /// Returns [`Stream`] which will provided `Some(())` every time when
    /// [`SessionState`] goes to the [`SessionState::Lost`].
    ///
    /// [`Stream`]: futures::Stream
    fn on_connection_loss(&self) -> LocalBoxStream<'static, ()> {
        let can_reconnect = Rc::clone(&self.can_reconnect);
        self.state
            .subscribe()
            .filter_map(move |state| {
                if matches!(state, SessionState::Lost(_, _))
                    && can_reconnect.get()
                {
                    future::ready(Some(()))
                } else {
                    future::ready(None)
                }
            })
            .boxed_local()
    }

    /// Returns [`Stream`] which will provided `Some(())` every time when
    /// [`SessionState`] goes to the [`SessionState::Opened`].
    ///
    /// Nothing will be provided if [`SessionState`] goes to the
    /// [`SessionState::Opened`] first time.
    ///
    /// [`Stream`]: futures::Stream
    fn on_reconnected(&self) -> LocalBoxStream<'static, ()> {
        let can_reconnect = Rc::clone(&self.can_reconnect);
        self.state
            .subscribe()
            .filter_map(move |current_state| {
                let can_reconnect = Rc::clone(&can_reconnect);
                async move {
                    if matches!(current_state, SessionState::Opened(_))
                        && can_reconnect.get()
                    {
                        Some(())
                    } else {
                        None
                    }
                }
            })
            .boxed_local()
    }
}

impl RpcEventHandler for WebSocketRpcSession {
    type Output = ();

    /// If current [`SessionState`] is [`SessionState::Authorizing`] and
    /// [`RoomId`] from [`ConnectionInfo`] is equal to the provided
    /// [`RoomId`], then [`SessionState`] will be transited to the
    /// [`SessionState::Opened`].
    fn on_joined_room(&self, room_id: RoomId, member_id: MemberId) {
        let state = self.state.get();
        if let SessionState::Authorizing(info) = state {
            if info.room_id == room_id && info.member_id == member_id {
                self.state.set(SessionState::Opened(info));
            }
        }
    }

    /// If current [`SessionState`] is [`SessionState::Opened`] or
    /// [`SessionState::Authorizing`] and provided [`RoomId`] is
    /// equal to the [`RoomId`] from the [`ConnectionInfo`] of this
    /// [`WebSocketRpcSession`], then [`SessionState`] will be transited
    /// to the [`SessionState::Finished`] if current [`SessionState`] is
    /// [`SessionState::Opened`] or to the [`SessionState::Uninitialized`] if
    /// current [`SessionState`] is [`SessionState::Authorizing`].
    fn on_left_room(&self, room_id: RoomId, close_reason: CloseReason) {
        let state = self.state.get();

        match &state {
            SessionState::Opened(info) | SessionState::Authorizing(info) => {
                if info.room_id != room_id {
                    return;
                }
            }
            _ => return,
        }

        match state {
            SessionState::Opened(_) => {
                self.state.set(SessionState::Finished(close_reason));
            }
            SessionState::Authorizing(_) => {
                self.state.set(SessionState::Uninitialized);
            }
            _ => {}
        }
    }

    /// Sends received [`Event`] to the all [`RpcSession::subscribe`]
    /// subscribers if current [`SessionState`] is [`SessionState::Opened`]
    /// and provided [`RoomId`] is equal to the [`RoomId`] from the
    /// [`ConnectionInfo`].
    fn on_event(&self, room_id: RoomId, event: Event) {
        if let SessionState::Opened(info) = self.state.get() {
            if info.room_id == room_id {
                self.event_txs
                    .borrow_mut()
                    .retain(|tx| tx.unbounded_send(event.clone()).is_ok());
            }
        }
    }
}

/// State for the [`WebSocketRpcSession`].
///
/// # State transition scheme
///
/// ```text
///      +---------------+
/// +--->+ Uninitialized |
/// |    +-------+-------+
/// |            |
/// |            v
/// |    +-------+-------+
/// |    |  Initialized  |
/// |    +-------+-------+
/// |            |
/// |            v                    +------------+
/// |    +-------+-------+<-----------+            |
/// |    |  Connecting   |            |   Failed   |
/// |    +-------+-------+----------->+            |
/// |            |                    +------+-----+
/// |            v                           ^
/// |    +-------+-------+                   |
/// +----+  Authorizing  +------------------>+
///      +-------+-------+                   |
///              |                           |
///              v                           |
///      +-------+-------+                   |
///      |    Opened     +------------------>+
///      +-------+-------+
///              |
///              v
///      +-------+-------+
///      |   Finished    |
///      +---------------+
/// ```
#[derive(Clone, Debug, Derivative)]
#[derivative(PartialEq)]
pub enum SessionState {
    /// [`WebSocketRpcSession`] currently doesn't have [`ConnectionInfo`] to
    /// authorize with.
    Uninitialized,

    /// [`ConnectionInfo`] was specified, but no transport connection
    /// establishment attempts were made yet.
    Initialized(Rc<ConnectionInfo>),

    /// Ongoing transport connection establishment.
    Connecting(Rc<ConnectionInfo>),

    /// Transport connection is establish and [`WebSocketRpcSession`] is
    /// currently performing session authorization.
    Authorizing(Rc<ConnectionInfo>),

    /// Connection with a server was lost but can be recovered.
    Lost(
        #[derivative(PartialEq = "ignore")] Rc<Traced<SessionError>>,
        Rc<ConnectionInfo>,
    ),

    /// Connection with a server is established and [`WebSocketRpcSession`] is
    /// authorized.
    Opened(Rc<ConnectionInfo>),

    /// Terminal state: transport is closed and can not be reopened.
    Finished(CloseReason),
}

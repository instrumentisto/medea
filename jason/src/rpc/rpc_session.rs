use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use async_trait::async_trait;
use derivative::Derivative;
use derive_more::{Display, From};
use futures::{
    channel::{mpsc, oneshot, oneshot::Canceled},
    future::LocalBoxFuture,
    stream::LocalBoxStream,
    StreamExt,
};
use medea_client_api_proto::{Command, Event, MemberId, RoomId};
use medea_macro::dispatchable;
use medea_reactive::ObservableCell;
use tracerr::Traced;
use wasm_bindgen_futures::spawn_local;

use crate::{
    rpc::{
        websocket::RpcEventHandler, ClientDisconnect, CloseReason,
        ConnectionInfo, RpcClientError, WebSocketRpcClient,
    },
    utils::{JsCaused, JsError},
};

/// Errors which are can be returned from the [`WebSocketRpcSession`].
#[derive(Clone, Debug, From, JsCaused, Display)]
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
    #[display(fmt = "RPC Session authorization on the server was failed")]
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

/// State for the [`WebSocketRpcSession`].
///
/// # State transition scheme
///
/// ```text
/// +---------------+
/// | Uninitialized |
/// +-------+-------+
///         |
///         v
/// +-------+-------+            +--------+
/// |  Initialized  +<-----------+ Failed |
/// +-------+-------+            +----+---+
///         |                         ^
///         v                         |
/// +-------+-------+                 |
/// |  Connecting   +-----------------+
/// +-------+-------+                 |
///         |                         |
///         v                         |
/// +-------+-------+                 |
/// |  Authorizing  +-----------------+
/// +-------+-------+                 |
///         |                         |
///         v                         |
/// +-------+-------+                 |
/// |    Opened     +-----------------+
/// +-------+-------+
///         |
///         v
/// +-------+-------+
/// |   Finished    |
/// +---------------+
/// ```
#[dispatchable(self: Rc<Self>, async_trait(?Send))]
#[derive(Clone, Debug, Derivative)]
#[derivative(PartialEq)]
enum SessionState {
    /// [`WebSocketRpcSession`] currently doesn't have [`ConnectionInfo`] to
    /// authorize with.
    Uninitialized,

    /// [`WebSocketRpcSession`] has [`ConnectionInfo`], but connection with a
    /// server currently doesn't established.
    Initialized(Rc<ConnectionInfo>),

    /// [`WebSocketRpcSession`] connecting to the server.
    Connecting(Rc<ConnectionInfo>),

    /// [`WebSocketRpcSession`] is connected to the server and authorizing on
    /// it.
    Authorizing(Rc<ConnectionInfo>),

    /// Connection with a server was lost.
    Failed(
        #[derivative(PartialEq = "ignore")] Rc<Traced<SessionError>>,
        Rc<ConnectionInfo>,
    ),

    /// Connection with a server is established and [`WebSocketRpcSession`] is
    /// authorized.
    Opened(Rc<ConnectionInfo>),

    /// Session with a server was finished.
    ///
    /// [`WebSocketRpcSession`] can't be used.
    Finished(CloseReason),
}

/// Tries to upgrade [`Weak`].
///
/// Breaks cycle if [`Weak`] is [`None`].
macro_rules! upgrade_or_break {
    ($weak:tt) => {
        if let Some(this) = $weak.upgrade() {
            this
        } else {
            break;
        }
    };
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
    /// function will not perform one more connection try. It will subsribe
    /// to [`SessionState`] changes and wait for first connection result.
    /// And based on this result - this function will be resolved.
    ///
    /// If [`RpcSession`] already in [`SessionState::Open`] then this function
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
    fn on_normal_close(
        &self,
    ) -> LocalBoxFuture<'static, Result<CloseReason, oneshot::Canceled>>;

    /// Sets reason, that will be passed to underlying transport when this
    /// client will be dropped.
    fn close_with_reason(&self, close_reason: ClientDisconnect);

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
}

/// Client to talk with server via Client API RPC.
///
/// Responsible for [`Room`] authorization and closing.
pub struct WebSocketRpcSession {
    /// [WebSocket] based Rpc Client used to .
    ///
    /// [WebSocket]: https://developer.mozilla.org/ru/docs/WebSockets
    client: Rc<WebSocketRpcClient>,

    /// Current [`SessionState`] of this [`WebSocketRpcSession`].
    state: ObservableCell<SessionState>,

    /// Flag which indicates that [`WebSocketRpcSession`] goes to the
    /// [`SessionState::Failed`] from the [`SessionState::Open`].
    is_can_be_reconnected: Rc<Cell<bool>>,

    /// Subscribers of the [`RpcSession::subscribe`].
    event_txs: RefCell<Vec<mpsc::UnboundedSender<Event>>>,
}

impl WebSocketRpcSession {
    /// Returns new uninitialized [`WebSocketRpcSession`] with a provided
    /// [`WebSocketRpcClient`].
    ///
    /// Spawns all [`WebSocketpRpcSession`] task.
    pub fn new(client: Rc<WebSocketRpcClient>) -> Rc<Self> {
        let this = Rc::new(Self {
            client,
            state: ObservableCell::new(SessionState::Uninitialized),
            is_can_be_reconnected: Rc::new(Cell::new(false)),
            event_txs: RefCell::default(),
        });
        this.spawn_tasks();

        this
    }

    /// Spawns tasks important for [`WebSocketRpcSession`] work.
    fn spawn_tasks(self: &Rc<Self>) {
        self.spawn_state_watcher();
        self.spawn_connection_loss_watcher();
        self.spawn_close_watcher();
        self.spawn_server_msg_listener();
    }

    /// Tries to establish connection with a server.
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
    /// Errors with [`SessionError`] if
    /// [`WebSocketRpcSession::wait_for_connect`] returned error.
    async fn connect(self: Rc<Self>) -> Result<(), Traced<SessionError>> {
        use SessionError as E;
        use SessionState as S;

        let current_state = self.state.clone_inner();
        match current_state {
            S::Connecting(_) | S::Authorizing(_) | S::Opened(_) => (),
            S::Initialized(info) | S::Failed(_, info) => {
                self.state.set(S::Connecting(info));
            }
            S::Uninitialized => {
                return Err(tracerr::new!(E::NoCredentials));
            }
            S::Finished(reason) => {
                return Err(tracerr::new!(E::SessionFinished(reason)));
            }
        }

        self.wait_for_connect()
            .await
            .map_err(tracerr::map_from_and_wrap!())
    }

    /// Waits for [`WebSocketRpcSession`] connecting result.
    ///
    /// # Errors
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
    /// [`SessionState::Failed`].
    async fn wait_for_connect(
        self: Rc<Self>,
    ) -> Result<(), Traced<SessionError>> {
        use SessionError as E;
        use SessionState as S;

        let mut state_updates_stream = self.state.subscribe();
        while let Some(state) = state_updates_stream.next().await {
            match state {
                S::Initialized(_) => {
                    return Err(tracerr::new!(E::NewConnectionInfo));
                }
                S::Opened(_) => return Ok(()),
                S::Failed(err, _) => {
                    // TODO: Clone Traced and add new Frame to it when Traced
                    //       cloning will be implemented.
                    return Err(tracerr::new!(AsRef::<SessionError>::as_ref(
                        &err.as_ref()
                    )
                    .clone()));
                }
                S::Uninitialized => {
                    return Err(tracerr::new!(E::AuthorizationFailed))
                }
                S::Finished(reason) => {
                    return Err(tracerr::new!(E::SessionFinished(reason)));
                }
                _ => (),
            }
        }

        Err(tracerr::new!(E::SessionUnexpectedlyDropped))
    }

    /// Handler for the [`WebSocketRpcClient::on_connection_loss`].
    ///
    /// Sets [`WebSocketRpcSession::state`] to the [`SessionState::Failed`].
    fn connection_lost(&self) {
        use SessionState as S;

        let current_state = self.state.clone_inner();
        if matches!(current_state, S::Opened(_)) {
            self.is_can_be_reconnected.set(true);
        }
        match current_state {
            S::Connecting(info) | S::Authorizing(info) | S::Opened(info) => {
                self.state.set(S::Failed(
                    Rc::new(tracerr::new!(SessionError::ConnectionLost)),
                    info,
                ));
            }
            S::Uninitialized
            | S::Initialized(_)
            | S::Failed(_, _)
            | S::Finished(_) => {}
        }
    }

    /// Spawns [`SessionState`] updates handler for this
    /// [`WebSocketRpcSession`].
    fn spawn_state_watcher(self: &Rc<Self>) {
        spawn_local({
            let weak_this = Rc::downgrade(self);
            let mut state_updates = self.state.subscribe();
            async move {
                while let Some(state) = state_updates.next().await {
                    let this = upgrade_or_break!(weak_this);
                    state.dispatch_with(this).await;
                }
            }
        });
    }

    /// Spawns [`WebSocketRpcClient::on_connection_loss`] listener.
    fn spawn_connection_loss_watcher(self: &Rc<Self>) {
        spawn_local({
            let weak_this = Rc::downgrade(self);
            let mut client_on_connection_loss =
                self.client.on_connection_loss();

            async move {
                while client_on_connection_loss.next().await.is_some() {
                    let this = upgrade_or_break!(weak_this);
                    this.connection_lost();
                }
            }
        });
    }

    /// Spawns [`WebSocketRpcClient::on_normal_close`] listener.
    fn spawn_close_watcher(self: &Rc<Self>) {
        spawn_local({
            let weak_this = Rc::downgrade(self);
            let on_normal_close = self.client.on_normal_close();
            async move {
                let reason = on_normal_close.await.unwrap_or_else(|_| {
                    ClientDisconnect::RpcClientUnexpectedlyDropped.into()
                });
                if let Some(this) = weak_this.upgrade() {
                    this.state.set(SessionState::Finished(reason));
                }
            }
        });
    }

    /// Spawns [`WebSocketRpcClient::subscribe`] listener.
    fn spawn_server_msg_listener(self: &Rc<Self>) {
        let mut server_msg_rx = self.client.subscribe();
        let weak_this = Rc::downgrade(self);
        spawn_local(async move {
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
        match self.state.clone_inner() {
            S::Uninitialized | S::Initialized(_) | S::Failed(_, _) => {
                self.state.set(S::Initialized(Rc::new(connection_info)));
            }
            S::Finished(reason) => {
                return Err(tracerr::new!(SessionError::SessionFinished(
                    reason
                )));
            }
            S::Connecting(info) | S::Authorizing(info) | S::Opened(info) => {
                if info.as_ref() != &connection_info {
                    self.state.set(S::Initialized(Rc::new(connection_info)));
                }
            }
        }

        self.connect()
            .await
            .map_err(tracerr::map_from_and_wrap!())?;

        Ok(())
    }

    /// Tries to reconnect this [`WebSocketRpcSession`] to the server.
    async fn reconnect(self: Rc<Self>) -> Result<(), Traced<SessionError>> {
        self.connect()
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
    fn send_command(&self, command: Command) {
        let current_state = self.state.clone_inner();
        if let SessionState::Opened(info) = current_state {
            self.client.send_command(info.room_id.clone(), command);
        } else {
            log::error!(
                "Tried to send Command while RPC Sesssion is in {:?} state",
                current_state
            );
        }
    }

    /// Returns [`Future`] which will be resolved when [`SessionState`] will be
    /// transited to the [`SessionState::Finished`] or [`WebSocketRpcSession`]
    /// will be dropped.
    fn on_normal_close(
        &self,
    ) -> LocalBoxFuture<'static, Result<CloseReason, Canceled>> {
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
            Ok(state_stream.next().await.unwrap_or_else(|| {
                ClientDisconnect::SessionUnexpectedlyDropped.into()
            }))
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
        if let SessionState::Opened(info) = self.state.clone_inner() {
            self.client
                .leave_room(info.room_id.clone(), info.member_id.clone());
        }

        self.client.set_close_reason(close_reason);
        self.state.set(SessionState::Finished(close_reason.into()));
    }

    /// Returns [`Stream`] which will provided `Some(())` every time when
    /// [`SessionState`] goes to the [`SessionState::Failed`].
    fn on_connection_loss(&self) -> LocalBoxStream<'static, ()> {
        self.state
            .subscribe()
            .filter_map(|state| async move {
                if matches!(state, SessionState::Failed(_, _)) {
                    Some(())
                } else {
                    None
                }
            })
            .boxed_local()
    }

    /// Returns [`Stream`] which will provided `Some(())` every time when
    /// [`SessionState`] goes to the [`SessionState::Opened`].
    ///
    /// Nothing will be provided if [`SessionState`] goes to the
    /// [`SessionState::Opened`] first time.
    fn on_reconnected(&self) -> LocalBoxStream<'static, ()> {
        let is_can_be_reconnected = Rc::clone(&self.is_can_be_reconnected);
        self.state
            .subscribe()
            .filter_map(move |current_state| {
                let is_can_be_reconnected = is_can_be_reconnected.clone();
                async move {
                    if matches!(current_state, SessionState::Opened(_))
                        && is_can_be_reconnected.get()
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
    type Output = Option<()>;

    /// If current [`SessionState`] is [`SessionState::Authorizing`] and
    /// [`RoomId`] from [`ConnectionInfo`] is equal to the provided
    /// [`RoomId`], then [`SessionState`] will be transited to the
    /// [`SessionState::Opened`].
    fn on_joined_room(
        &self,
        room_id: RoomId,
        member_id: MemberId,
    ) -> Self::Output {
        if let SessionState::Authorizing(info) = self.state.clone_inner() {
            if info.room_id == room_id && info.member_id == member_id {
                self.state.set(SessionState::Opened(info));
            }
        }

        Some(())
    }

    /// If current [`SessionState`] is [`SessionState::Opened`] or
    /// [`SessionState::Authorizing`] and provided [`RoomId`] is
    /// equal to the [`RoomId`] from the [`ConnectionInfo`] of this
    /// [`WebSocketRpcSession`], then [`SessionState`] will be transited
    /// to the [`SessionState::Finished`] if current [`SessionState`] is
    /// [`SessionState::Opened`] or to the [`SessionState::Uninitialized`] if
    /// current [`SessionState`] is [`SessionState::Authorizing`].
    fn on_left_room(
        &self,
        room_id: RoomId,
        close_reason: CloseReason,
    ) -> Self::Output {
        let current_state = self.state.clone_inner();

        match &current_state {
            SessionState::Opened(info) | SessionState::Authorizing(info) => {
                if info.room_id != room_id {
                    return None;
                }
            }
            _ => return None,
        }

        match current_state {
            SessionState::Opened(_) => {
                self.state.set(SessionState::Finished(close_reason));
            }
            SessionState::Authorizing(_) => {
                self.state.set(SessionState::Uninitialized);
            }
            _ => (),
        }

        Some(())
    }

    /// Sends received [`Event`] to the all [`RpcSession::subscribe`]
    /// subscribers if current [`SessionState`] is [`SessionState::Opened`]
    /// and provided [`RoomId`] is equal to the [`RoomId`] from the
    /// [`ConnectionInfo`].
    fn on_event(&self, room_id: RoomId, event: Event) -> Self::Output {
        if let SessionState::Opened(info) = self.state.clone_inner() {
            if info.room_id == room_id {
                self.event_txs
                    .borrow_mut()
                    .retain(|tx| tx.unbounded_send(event.clone()).is_ok());
            }
        }

        Some(())
    }
}

#[async_trait(?Send)]
impl SessionStateHandler for WebSocketRpcSession {
    type Output = ();

    /// No-op.
    async fn on_uninitialized(self: Rc<Self>) {}

    /// No-op.
    async fn on_initialized(self: Rc<Self>, _: Rc<ConnectionInfo>) {}

    /// Tries to connect to the server with a [`WebSocketRpcClient::connect`].
    ///
    /// Sets [`SessionState`] to the [`SessionState::Authorizing`] if
    /// [`WebSocketRpcClient`] successfully connected to the server.
    ///
    /// Sets [`SessionState`] to the [`SessionState::Failed`] if
    /// [`WebSocketRpcClient::connect`] errored.
    async fn on_connecting(self: Rc<Self>, info: Rc<ConnectionInfo>) {
        match Rc::clone(&self.client)
            .connect(info.url.clone())
            .await
            .map_err(tracerr::map_from_and_wrap!())
        {
            Ok(_) => {
                self.state.set(SessionState::Authorizing(info));
            }
            Err(e) => {
                self.state.set(SessionState::Failed(Rc::new(e), info));
            }
        }
    }

    /// No-op.
    async fn on_failed(
        self: Rc<Self>,
        _: (Rc<Traced<SessionError>>, Rc<ConnectionInfo>),
    ) {
    }

    /// Authorizes [`WebSocketRpcSession`] on server with
    /// [`WebSocketRpcClient::authorize`].
    ///
    /// Doesn't updates [`SessionState`]. [`SessionState`] will be updated when
    /// [`ServerMsg::JoinedRoom`] will be received.
    async fn on_authorizing(self: Rc<Self>, info: Rc<ConnectionInfo>) {
        self.client.authorize(
            info.room_id.clone(),
            info.member_id.clone(),
            info.credential.clone(),
        );
    }

    /// No-op.
    async fn on_opened(self: Rc<Self>, _: Rc<ConnectionInfo>) {}

    /// No-op.
    async fn on_finished(self: Rc<Self>, _: CloseReason) {}
}

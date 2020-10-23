use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use async_trait::async_trait;
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

#[derive(Clone, Debug, From, JsCaused, Display)]
pub enum SessionError {
    #[display(fmt = "Session finished with {:?} close reason", _0)]
    SessionFinished(CloseReason),

    #[display(fmt = "Session doesn't have any credentials to authorize with")]
    NoCredentials,

    #[display(fmt = "Session authorization on the server was failed")]
    AuthorizationFailed,

    #[display(fmt = "RpcClientError: {:?}", _0)]
    RpcClient(#[js(cause)] RpcClientError),

    #[display(fmt = "Session was unexpectedly dropped")]
    SessionUnexpectedlyDropped,

    #[display(fmt = "Connection with a server was lost")]
    ConnectionLost,

    #[display(fmt = "Session state currently is not New")]
    NotNew,

    #[display(fmt = "New connection info was provided")]
    NewConnectionInfo,
}

#[dispatchable(self: Rc<Self>, async_trait(?Send))]
#[derive(Clone, Debug)]
enum SessionState {
    Uninitialized,
    Initialized(Rc<ConnectionInfo>),
    Connecting(Rc<ConnectionInfo>),
    Authorizing(Rc<ConnectionInfo>),
    Failed(Rc<Traced<SessionError>>, Rc<ConnectionInfo>),
    Opened(Rc<ConnectionInfo>),
    Finished(CloseReason),
}

impl PartialEq for SessionState {
    fn eq(&self, other: &Self) -> bool {
        use SessionState as S;
        match (self, other) {
            (S::Uninitialized, S::Uninitialized) => true,
            (S::Initialized(a), S::Initialized(b)) => a == b,
            (S::Connecting(a), S::Connecting(b)) => a == b,
            (S::Failed(_, a), S::Failed(_, b)) => a == b,
            (S::Authorizing(a), S::Authorizing(b)) => a == b,
            (S::Opened(a), S::Opened(b)) => a == b,
            (S::Finished(a), S::Finished(b)) => a == b,
            _ => false,
        }
    }
}

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
    /// Tries to upgrade [`State`] of this [`RpcSession`] to [`State::Open`].
    ///
    /// This function is also used for reconnection of this [`RpcClient`].
    ///
    /// If [`RpcSession`] is closed than this function will try to establish
    /// new RPC connection.
    ///
    /// If [`RpcSession`] already in [`State::Connecting`] then this function
    /// will not perform one more connection try. It will subsribe to
    /// [`State`] changes and wait for first connection result. And based on
    /// this result - this function will be resolved.
    ///
    /// If [`RpcSession`] already in [`State::Open`] then this function will be
    /// instantly resolved.
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

    state: ObservableCell<SessionState>,

    is_can_be_reconnected: Rc<Cell<bool>>,

    event_txs: RefCell<Vec<mpsc::UnboundedSender<Event>>>,
}

impl WebSocketRpcSession {
    /// Returns new uninitialized [`WebSocketRpcSession`] with a provided
    /// [`WebSocketRpcClient`].
    pub fn new(client: Rc<WebSocketRpcClient>) -> Rc<Self> {
        let this = Rc::new(Self {
            client,
            state: ObservableCell::new(SessionState::Uninitialized),
            is_can_be_reconnected: Rc::new(Cell::new(false)),
            event_txs: RefCell::default(),
        });

        this.spawn_state_watcher();
        this.spawn_connection_loss_watcher();
        this.spawn_close_watcher();
        this.spawn_server_msg_listener();

        this
    }

    async fn connect(self: Rc<Self>) -> Result<(), Traced<SessionError>> {
        use SessionError as E;
        use SessionState as S;

        let current_state = self.state.clone_inner();
        match current_state {
            S::Connecting(_) | S::Authorizing(_) | S::Opened(_) => (),
            S::Initialized(info) => {
                self.state.set(S::Connecting(info));
            }
            S::Failed(_, info) => {
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
                    self.state
                        .set(S::Initialized(Rc::new(connection_info)));
                }
            }
        }

        self.connect()
            .await
            .map_err(tracerr::map_from_and_wrap!())?;

        Ok(())
    }

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

    fn send_command(&self, command: Command) {
        if let SessionState::Opened(info) = self.state.clone_inner() {
            self.client.send_command(info.room_id.clone(), command);
        } else {
            log::error!("Tried to send command in disconnected state");
        }
    }

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

    fn close_with_reason(&self, close_reason: ClientDisconnect) {
        match self.state.clone_inner() {
            SessionState::Opened(info) => {
                self.client
                    .leave_room(info.room_id.clone(), info.member_id.clone());
            }
            _ => (),
        }

        self.client.set_close_reason(close_reason);
        self.state.set(SessionState::Finished(close_reason.into()));
    }

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

    async fn on_uninitialized(self: Rc<Self>) {}

    async fn on_initialized(self: Rc<Self>, _: Rc<ConnectionInfo>) {}

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

    async fn on_failed(
        self: Rc<Self>,
        _: (Rc<Traced<SessionError>>, Rc<ConnectionInfo>),
    ) {
    }

    async fn on_authorizing(self: Rc<Self>, info: Rc<ConnectionInfo>) {
        self.client.authorize(
            info.room_id.clone(),
            info.member_id.clone(),
            info.credential.clone(),
        );
    }

    async fn on_opened(self: Rc<Self>, _: Rc<ConnectionInfo>) {}

    async fn on_finished(self: Rc<Self>, _: CloseReason) {}
}

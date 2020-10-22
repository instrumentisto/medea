use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use async_trait::async_trait;
use derive_more::From;
use futures::{
    channel::{oneshot, oneshot::Canceled},
    future,
    future::{Either, LocalBoxFuture},
    stream::LocalBoxStream,
    StreamExt,
};
use medea_client_api_proto::{Command, Event, MemberId, RoomId};
use medea_reactive::ObservableCell;
use tracerr::Traced;
use wasm_bindgen_futures::spawn_local;

use crate::rpc::{
    rpc_session::SessionError::RpcClient,
    websocket::{RpcEvent, RpcEventHandler},
    ApiUrl, ClientDisconnect, CloseReason, ConnectionInfo, RpcClientError,
    WebSocketRpcClient,
};

#[derive(Debug, From)]
pub enum SessionError {
    SessionFinished(CloseReason),

    NoCredentials,

    AuthorizationFailed,

    RpcClient(RpcClientError),

    ConnectionFailed,
}

// // TODO: add Debug derive
#[derive(Clone, PartialEq, Eq, Debug)]
enum ConnectedSessionState {
    Open,
    Authorizing,
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct ConnectedSession {
    info: Rc<ConnectionInfo>,
    state: ConnectedSessionState,
}

impl ConnectedSession {
    fn new(state: ConnectedSessionState, info: Rc<ConnectionInfo>) -> Self {
        Self { info, state }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum SessionState {
    New,
    ReadyForConnect(Rc<ConnectionInfo>),
    Connecting(ConnectedSession),
    // TODO: provide RpcClientError
    Failed(ConnectedSession),
    Connected(ConnectedSession),
    Finished(CloseReason),
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
    ) -> Result<(), Traced<RpcClientError>>;

    /// Tries to reconnect (or connect) this [`RpcSession`] to the Media Server.
    async fn reconnect(self: Rc<Self>) -> Result<(), Traced<RpcClientError>>;

    /// Returns [`Stream`] of all [`Event`]s received by this [`RpcSession`].
    ///
    /// [`Stream`]: futures::Stream
    fn subscribe(self: Rc<Self>) -> LocalBoxStream<'static, Event>;

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
}

impl WebSocketRpcSession {
    /// Returns new uninitialized [`WebSocketRpcSession`] with a provided
    /// [`WebSocketRpcClient`].
    pub fn new(client: Rc<WebSocketRpcClient>) -> Rc<Self> {
        let this = Rc::new(Self {
            client,
            state: ObservableCell::new(SessionState::New),
        });

        spawn_local({
            let mut state_updates = this.state.subscribe();
            let weak_this = Rc::downgrade(&this);
            async move {
                while let Some(state) = state_updates.next().await {
                    // TODO: unwrap
                    let this = weak_this.upgrade().unwrap();
                    match state {
                        SessionState::Connecting(desired_state) => {
                            match Rc::clone(&this.client)
                                .connect(desired_state.info.url.clone())
                                .await
                            {
                                Ok(_) => {
                                    this.state.set(SessionState::Connected(
                                        desired_state,
                                    ));
                                }
                                Err(_) => {
                                    this.state.set(SessionState::Failed(
                                        desired_state,
                                    ));
                                }
                            }
                        }
                        SessionState::Connected(state) => match state.state {
                            ConnectedSessionState::Authorizing => {
                                this.client.authorize(
                                    state.info.room_id.clone(),
                                    state.info.member_id.clone(),
                                    state.info.credential.clone(),
                                );
                            }
                            ConnectedSessionState::Open => (),
                        },
                        _ => (),
                    }
                }
            }
        });

        spawn_local({
            let weak_this = Rc::downgrade(&this);
            let mut client_on_connection_loss =
                this.client.on_connection_loss();

            async move {
                while client_on_connection_loss.next().await.is_some() {
                    // TODO: unwrap
                    let this = weak_this.upgrade().unwrap();
                    let current_state = this.state.clone_inner();
                    match current_state {
                        SessionState::Connecting(state)
                        | SessionState::Connected(state) => {
                            this.state.set(SessionState::Failed(state));
                        }
                        SessionState::New
                        | SessionState::ReadyForConnect(_)
                        | SessionState::Failed(_)
                        | SessionState::Finished(_) => {}
                    }
                }
            }
        });

        spawn_local({
            let weak_this = Rc::downgrade(&this);
            let mut on_normal_close = this.client.on_normal_close();
            async move {
                let reason = on_normal_close.await.unwrap_or_else(|_| {
                    ClientDisconnect::RpcTransportUnexpectedlyDropped.into()
                });
                let this = weak_this.upgrade().unwrap();
                this.state.set(SessionState::Finished(reason));
            }
        });

        this
    }

    async fn connect(self: Rc<Self>) -> Result<(), SessionError> {
        let current_state = self.state.clone_inner();
        match current_state {
            SessionState::Connecting(_) | SessionState::Connected(_) => (),
            SessionState::ReadyForConnect(info) => {
                self.state.set(SessionState::Connecting(
                    ConnectedSession::new(
                        ConnectedSessionState::Authorizing,
                        info,
                    ),
                ));
            }
            SessionState::Failed(state) => {
                self.state.set(SessionState::Connecting(state));
            }
            SessionState::New => {
                return Err(SessionError::NoCredentials);
            }
            SessionState::Finished(reason) => {
                return Err(SessionError::SessionFinished(reason));
            }
        }

        let mut state_updates_stream = self.state.subscribe();
        while let Some(state) = state_updates_stream.next().await {
            match state {
                SessionState::Connected(state) => match state.state {
                    ConnectedSessionState::Open => return Ok(()),
                    _ => (),
                },
                SessionState::Failed(_) => {
                    return Err(SessionError::ConnectionFailed)
                }
                SessionState::New => {
                    return Err(SessionError::AuthorizationFailed)
                }
                SessionState::Finished(reason) => {
                    return Err(SessionError::SessionFinished(reason));
                }
                _ => (),
            }
        }

        Err(SessionError::ConnectionFailed)
    }
}

#[async_trait(?Send)]
impl RpcSession for WebSocketRpcSession {
    async fn connect(
        self: Rc<Self>,
        connection_info: ConnectionInfo,
    ) -> Result<(), Traced<RpcClientError>> {
        self.state
            .set(SessionState::ReadyForConnect(Rc::new(connection_info)));
        // TODO
        self.connect().await.unwrap();

        Ok(())
    }

    async fn reconnect(self: Rc<Self>) -> Result<(), Traced<RpcClientError>> {
        // TODO
        self.connect().await.unwrap();

        Ok(())
    }

    fn subscribe(self: Rc<Self>) -> LocalBoxStream<'static, Event> {
        let weak_this = Rc::downgrade(&self);
        Box::pin(self.client.subscribe().filter_map(move |event| {
            let weak_this = weak_this.clone();
            async move {
                let this = weak_this.upgrade()?;
                event.dispatch_with(this.as_ref())
            }
        }))
    }

    fn send_command(&self, command: Command) {
        if let SessionState::Connected(state) = self.state.clone_inner() {
            if let ConnectedSessionState::Open = state.state {
                self.client
                    .send_command(state.info.room_id.clone(), command);
            } else {
                log::error!("Tries to send command before authorizing");
            }
        } else {
            log::error!("Tried to send command before connecting");
        }
    }

    fn on_normal_close(
        &self,
    ) -> LocalBoxFuture<'static, Result<CloseReason, Canceled>> {
        let mut state_stream = self.state.subscribe();
        let mut state_stream = state_stream
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
                ClientDisconnect::RpcClientUnexpectedlyDropped.into()
            }))
        })
    }

    // TODO: maybe close_with_reason?
    fn set_close_reason(&self, close_reason: ClientDisconnect) {
        match self.state.clone_inner() {
            SessionState::Connected(state) => {
                if let ConnectedSessionState::Open = state.state {
                    self.client.leave_room(
                        state.info.room_id.clone(),
                        state.info.member_id.clone(),
                    );
                }
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
                if matches!(state, SessionState::Failed(_)) {
                    Some(())
                } else {
                    None
                }
            })
            .boxed_local()
    }

    fn on_reconnected(&self) -> LocalBoxStream<'static, ()> {
        self.state
            .subscribe()
            .filter_map(|current_state| {
                let mut is_inited = false;
                async move {
                    if let SessionState::Connected(state) = current_state {
                        match state.state {
                            ConnectedSessionState::Open => {
                                if is_inited {
                                    Some(())
                                } else {
                                    is_inited = true;
                                    None
                                }
                            }
                            ConnectedSessionState::Authorizing => None,
                        }
                    } else {
                        None
                    }
                }
            })
            .boxed_local()
    }
}

impl RpcEventHandler for WebSocketRpcSession {
    type Output = Option<Event>;

    fn on_joined_room(
        &self,
        room_id: RoomId,
        member_id: MemberId,
    ) -> Self::Output {
        let current_state = self.state.clone_inner();
        if let SessionState::Connected(state) = current_state {
            if let ConnectedSessionState::Open = state.state {
                if &room_id == &state.info.room_id
                    && &member_id == &state.info.member_id
                {
                    self.state.set(SessionState::Connected(
                        ConnectedSession::new(
                            ConnectedSessionState::Open,
                            state.info,
                        ),
                    ));
                }
            }
        }
        None
    }

    fn on_left_room(
        &self,
        room_id: RoomId,
        close_reason: CloseReason,
    ) -> Self::Output {
        let current_state = self.state.clone_inner();
        if let SessionState::Connected(state) = current_state {
            match state.state {
                ConnectedSessionState::Open => {
                    if state.info.room_id == room_id {
                        self.state.set(SessionState::Finished(close_reason));
                    }
                }
                ConnectedSessionState::Authorizing => {
                    if state.info.room_id == room_id {
                        self.state.set(SessionState::New);
                    }
                }
            }
        }
        None
    }

    fn on_event(&self, room_id: RoomId, event: Event) -> Self::Output {
        let current_state = self.state.clone_inner();
        if let SessionState::Connected(state) = current_state {
            if let ConnectedSessionState::Open = state.state {
                if state.info.room_id == room_id {
                    Some(event)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    }
}

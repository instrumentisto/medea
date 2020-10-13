use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use async_trait::async_trait;
use derive_more::From;
use futures::{
    channel::{oneshot, oneshot::Canceled},
    future::LocalBoxFuture,
    stream::LocalBoxStream,
    StreamExt,
};
use medea_client_api_proto::{Command, Event};
use medea_reactive::ObservableCell;
use tracerr::Traced;
use wasm_bindgen_futures::spawn_local;

use crate::rpc::{
    websocket::RpcEvent, ClientDisconnect, CloseReason, ConnectionInfo,
    RpcClientError, WebSocketRpcClient,
};

/// Flag which indicates that [`RpcSession`] is reconnected to the Media Server.
#[derive(Clone, Copy, Debug, PartialEq, From)]
struct IsReconnected(bool);

/// [`RpcSession`] connection state.
#[derive(Clone, Copy, Debug, PartialEq)]
enum SessionState {
    /// [`RpcSession`] connecting to the Media Server.
    Connecting,

    /// Connection with the Media Server is opened.
    Open(IsReconnected),

    /// Connection with the Media Server is closed.
    Closed,
}

/// Client to talk with server via Client API RPC.
#[async_trait(?Send)]
#[cfg_attr(feature = "mockable", mockall::automock)]
pub trait RpcSession {
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
    async fn connect(
        self: Rc<Self>,
        connection_info: ConnectionInfo,
    ) -> Result<(), Traced<RpcClientError>>;

    /// Tries to reconnect (or connect) this [`RpcSession`] to the Media Server.
    async fn reconnect(self: Rc<Self>) -> Result<(), Traced<RpcClientError>>;

    /// Returns [`Stream`] of all [`Event`]s received by this [`RpcClient`].
    ///
    /// [`Stream`]: futures::Stream
    fn subscribe(self: Rc<Self>) -> LocalBoxStream<'static, Event>;

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
}

/// Client to talk with server via Client API RPC.
///
/// Responsible for [`Room`] authorization and closing.
pub struct Session {
    /// Client API RPC client to talk with server via [WebSocket].
    ///
    /// [WebSocket]: https://developer.mozilla.org/ru/docs/WebSockets
    client: Rc<WebSocketRpcClient>,

    /// Information about [`Session`] connection.
    ///
    /// If `None` then this [`Session`] currently is uninitialized.
    credentials: RefCell<Option<ConnectionInfo>>,

    /// [`Session`] connection state.
    state: ObservableCell<SessionState>,

    /// Flag which indicates that this [`Session`] was initially connected.
    initially_connected: Cell<bool>,
}

impl Session {
    /// Returns new uninitialized [`Session`] with a provided
    /// [`WebSocketRpcClient`].
    pub fn new(client: Rc<WebSocketRpcClient>) -> Rc<Self> {
        let this = Rc::new(Self {
            client,
            credentials: RefCell::new(None),
            state: ObservableCell::new(SessionState::Closed),
            initially_connected: Cell::new(false),
        });
        spawn_local({
            let weak_this = Rc::downgrade(&this);
            let mut client_on_connection_loss =
                this.client.on_connection_loss();
            async move {
                while client_on_connection_loss.next().await.is_some() {
                    if let Some(this) = weak_this.upgrade() {
                        this.state.set(SessionState::Closed);
                    } else {
                        break;
                    }
                }
            }
        });

        this
    }
}

impl Session {
    async fn connect_session(&self) -> Result<(), Traced<RpcClientError>> {
        if let Some(credentials) = self.credentials.borrow().as_ref() {
            if self.state.get() != SessionState::Closed {
                return Ok(());
            }
            self.state.set(SessionState::Connecting);
            self.client
                .clone()
                .connect(credentials.url().clone())
                .await
                .map_err(|e| {
                    self.state.set(SessionState::Closed);
                    e
                })?;
            self.client.authorize(
                credentials.room_id.clone(),
                credentials.member_id.clone(),
                credentials.credential.clone(),
            );
            self.state
                .subscribe()
                .filter(|s| {
                    futures::future::ready(matches!(s, SessionState::Open(_)))
                })
                .next()
                .await;
        }

        Ok(())
    }
}

#[async_trait(?Send)]
impl RpcSession for Session {
    async fn connect(
        self: Rc<Self>,
        connection_info: ConnectionInfo,
    ) -> Result<(), Traced<RpcClientError>> {
        self.credentials.replace(Some(connection_info));
        self.reconnect().await
    }

    async fn reconnect(self: Rc<Self>) -> Result<(), Traced<RpcClientError>> {
        if let Some(credentials) = self.credentials.borrow().as_ref() {
            Rc::clone(&self.client)
                .connect(credentials.url.clone())
                .await?;
            self.connect_session().await
        } else {
            Err(tracerr::new!(RpcClientError::NoSocket))
        }
    }

    fn subscribe(self: Rc<Self>) -> LocalBoxStream<'static, Event> {
        let weak_this = Rc::downgrade(&self);
        Box::pin(self.client.subscribe().filter_map(move |event| {
            let weak_this = weak_this.clone();
            async move {
                let this = weak_this.upgrade()?;
                let x = match (this.credentials.borrow().as_ref(), event) {
                    (Some(credentials), RpcEvent::Event { room_id, event }) => {
                        if credentials.room_id == room_id {
                            Some(event)
                        } else {
                            None
                        }
                    }
                    (
                        Some(credentials),
                        RpcEvent::JoinedRoom { room_id, member_id },
                    ) => {
                        if credentials.room_id == room_id
                            && credentials.member_id == member_id
                        {
                            this.state.set(SessionState::Open(
                                this.initially_connected.get().into(),
                            ));
                        }
                        None
                    }
                    _ => None,
                };
                x
            }
        }))
    }

    fn send_command(&self, command: Command) {
        if let Some(credentials) = self.credentials.borrow().as_ref() {
            self.client
                .send_command(credentials.room_id.clone(), command);
        } else {
            log::error!("Tried to send command before connecting")
        }
    }

    fn on_normal_close(
        &self,
    ) -> LocalBoxFuture<'static, Result<CloseReason, Canceled>> {
        self.client.on_normal_close()
    }

    fn set_close_reason(&self, close_reason: ClientDisconnect) {
        if let Some(credentials) = self.credentials.borrow().as_ref() {
            self.client.leave_room(
                credentials.room_id.clone(),
                credentials.member_id.clone(),
            );
        }
        self.client.set_close_reason(close_reason)
    }

    fn on_connection_loss(&self) -> LocalBoxStream<'static, ()> {
        self.state
            .subscribe()
            .skip(1)
            .filter_map(|state| {
                futures::future::ready(
                    Some(()).filter(|_| matches!(state, SessionState::Closed)),
                )
            })
            .boxed_local()
    }

    fn on_reconnected(&self) -> LocalBoxStream<'static, ()> {
        self.state
            .subscribe()
            .filter_map(|state| async move {
                if state == SessionState::Open(IsReconnected(true)) {
                    Some(())
                } else {
                    None
                }
            })
            .boxed_local()
    }
}

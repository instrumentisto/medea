use std::{cell::RefCell, rc::Rc};

use async_trait::async_trait;
use derive_more::From;
use medea_client_api_proto::{Command, Event, MemberId, RoomId, Token};

use crate::rpc::{
    ClientDisconnect, CloseReason, RpcClientError, RpcSession,
    WebSocketRpcClient,
};

use tracerr::Traced;
use url::Url;

use crate::rpc::websocket::RpcEvent;
use futures::{
    channel::oneshot::Canceled, future::LocalBoxFuture, stream::LocalBoxStream,
    StreamExt,
};
use medea_reactive::ObservableCell;
use std::cell::Cell;

struct ConnectionInfo {
    url: Url,
    room_id: RoomId,
    member_id: MemberId,
    token: Token,
}

#[derive(Clone, Copy, Debug, PartialEq, From)]
struct IsReconnected(bool);

#[derive(Clone, Copy, Debug, PartialEq)]
enum SessionState {
    Connecting,
    Open(IsReconnected),
    Closed,
}

pub struct Session {
    client: Rc<WebSocketRpcClient>,
    credentials: RefCell<Option<ConnectionInfo>>,
    state: ObservableCell<SessionState>,
    initialy_connected: Cell<bool>,
}

impl Session {
    pub fn new(client: Rc<WebSocketRpcClient>) -> Self {
        Self {
            client,
            credentials: RefCell::new(None),
            state: ObservableCell::new(SessionState::Closed),
            initialy_connected: Cell::new(false),
        }
    }
}

impl Session {
    async fn connect_session(&self) -> Result<(), Traced<RpcClientError>> {
        if let Some(credentials) = self.credentials.borrow().as_ref() {
            self.state.set(SessionState::Connecting);
            self.client
                .clone()
                .connect(credentials.url.clone())
                .await
                .map_err(|e| {
                    self.state.set(SessionState::Closed);
                    e
                })?;
            self.client.authorize(
                credentials.room_id.clone(),
                credentials.member_id.clone(),
                credentials.token.clone(),
            );
            use futures::StreamExt as _;
            self.state.subscribe().filter(|s| futures::future::ready(matches!(s, SessionState::Open(_)))).next().await;
        }

        Ok(())
    }
}

#[async_trait(?Send)]
impl RpcSession for Session {
    async fn connect(
        self: Rc<Self>,
        url: Url,
        room_id: RoomId,
        member_id: MemberId,
        token: Token,
    ) -> Result<(), Traced<RpcClientError>> {
        self.credentials.replace(Some(ConnectionInfo {
            url,
            room_id,
            member_id,
            token,
        }));
        self.reconnect().await
    }

    async fn reconnect(self: Rc<Self>) -> Result<(), Traced<RpcClientError>> {
        if let Some(credentials) = self.credentials.borrow().as_ref() {
            Rc::clone(&self.client)
                .connect(credentials.url.clone())
                .await?;
            self.connect_session().await
        } else {
            log::error!("Tried to send command before connecting");
            Err(tracerr::new!(RpcClientError::NoSocket))
        }
    }

    fn subscribe(self: Rc<Self>) -> LocalBoxStream<'static, Event> {
        let this = Rc::clone(&self);
        Box::pin(self.client.subscribe().filter_map(move |event| {
            let this = Rc::clone(&this);
            async move {
                match (this.credentials.borrow().as_ref(), event) {
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
                            this.state.set(SessionState::Open(this.initialy_connected.get().into()));
                        }
                        None
                    }
                    _ => None,
                }
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
            log::debug!("LEAVE ROOM");
            self.client.leave_room(
                credentials.room_id.clone(),
                credentials.member_id.clone(),
            );
        }
        self.client.set_close_reason(close_reason)
    }

    fn on_connection_loss(&self) -> LocalBoxStream<'static, ()> {
        self.client.on_connection_loss()
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

use std::rc::Rc;

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
use wasm_bindgen::__rt::core::cell::RefCell;

struct ConnectionInfo {
    url: Url,
    room_id: RoomId,
    member_id: MemberId,
    token: Token,
}

#[derive(Clone, Debug, PartialEq, From)]
struct IsReconnected(bool);

enum SessionState {
    Connecting,
    Open(IsReconnected),
    Closed,
}

pub struct Session {
    client: Rc<WebSocketRpcClient>,
    credentials: RefCell<Option<ConnectionInfo>>,
    state: ObservableCell<SessionState>,
}

impl Session {
    pub fn new(client: Rc<WebSocketRpcClient>) -> Self {
        Self {
            client,
            credentials: RefCell::new(None),
            state: ObservableCell::new(SessionState::Closed),
        }
    }
}

impl Session {
    async fn connect_session(&self) -> Result<(), Traced<RpcClientError>> {
        unimplemented!()
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
            Rc::clone(&self.client).connect(credentials.url.clone()).await?;
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
        self.client.set_close_reason(close_reason)
    }

    fn on_connection_loss(&self) -> LocalBoxStream<'static, ()> {
        self.client.on_connection_loss()
    }

    fn on_reconnected(&self) -> LocalBoxStream<'static, ()> {
        unimplemented!()
    }
}

//! Abstraction over RPC transport.

mod heartbeat;
mod websocket;

use std::{cell::RefCell, rc::Rc, vec};

use futures::{
    channel::{mpsc, oneshot},
    future::LocalBoxFuture,
    stream::LocalBoxStream,
};
use js_sys::Date;
use medea_client_api_proto::{
    ClientMsg, CloseDescription, CloseReason, Command, Event, ServerMsg,
};
use wasm_bindgen::prelude::*;
use web_sys::CloseEvent;

use crate::utils::WasmErr;

use self::{heartbeat::Heartbeat, websocket::WebSocket};

/// Connection with remote was closed.
#[derive(Debug)]
pub enum CloseMsg {
    /// Transport was gracefully closed by remote.
    ///
    /// Determines by close code `1000` and existence of
    /// [`RpcConnectionCloseReason`].
    Normal(u16, CloseReason),
    /// Connection was unexpectedly closed. Consider reconnecting.
    ///
    /// Unexpected close determines by close code != `1000` and for close code
    /// 1000 without reason. This is used because if connection lost then
    /// close code will be `1000` which is wrong.
    Disconnect(u16),
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
                    Self::Disconnect(code)
                }
            }
            _ => Self::Disconnect(code),
        }
    }
}

/// Reason of why Jason was closed.
///
/// This struct will be provided into `on_close_by_server` JS side callback.
#[wasm_bindgen]
pub struct ClosedByServerReason {
    reason: String,
}

impl ClosedByServerReason {
    /// Creates new [`ClosedByServerReason`] with provided [`CloseReason`]
    /// converted into [`String`].
    pub fn new(reason: &CloseReason) -> Self {
        Self {
            reason: reason.to_string(),
        }
    }
}

#[wasm_bindgen]
impl ClosedByServerReason {
    #[wasm_bindgen(getter)]
    pub fn reason(&self) -> String {
        self.reason.clone()
    }
}

// TODO: consider using async-trait crate, it doesnt work with mockall atm
/// Client to talk with server via Client API RPC.
#[allow(clippy::module_name_repetitions)]
#[cfg_attr(feature = "mockable", mockall::automock)]
pub trait RpcClient {
    // atm Establish connection with RPC server.
    fn connect(
        &self,
        token: String,
    ) -> LocalBoxFuture<'static, Result<(), WasmErr>>;

    /// Returns [`Stream`] of all [`Event`]s received by this [`RpcClient`].
    ///
    /// [`Stream`]: futures::Stream
    fn subscribe(&self) -> LocalBoxStream<'static, Event>;

    /// Unsubscribes from this [`RpcClient`]. Drops all subscriptions atm.
    fn unsub(&self);

    /// Sends [`Command`] to server.
    fn send_command(&self, command: Command);

    /// Sets `on_close_room` callback which will be called on [`Room`] close.
    ///
    /// [`Room`]: crate::api::room::Room
    fn on_close_by_server(
        &self,
    ) -> LocalBoxFuture<'static, Result<CloseReason, oneshot::Canceled>>;
}

// TODO:
// 1. Proper sub registry.
// 2. Reconnect.
// 3. Disconnect if no pongs.
// 4. Buffering if no socket?
pub struct WebsocketRpcClient(Rc<RefCell<Inner>>);

/// Inner state of [`RpcClient`].
struct Inner {
    /// WebSocket connection to remote media server.
    sock: Option<Rc<WebSocket>>,

    heartbeat: Heartbeat,

    /// Event's subscribers list.
    subs: Vec<mpsc::UnboundedSender<Event>>,

    /// [`oneshot::Sender`] with which [`CloseReason`] will be sent when
    /// WebSocket connection normally closed by server.
    ///
    /// Note that [`CloseReason`] will not be sent if WebSocket closed with
    /// [`RpcConnectionCloseReason::NewConnection`] reason.
    on_close_by_server_subscribers: Vec<oneshot::Sender<CloseReason>>,
}

impl Inner {
    fn new(heartbeat_interval: i32) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            sock: None,
            on_close_by_server_subscribers: Vec::new(),
            subs: vec![],
            heartbeat: Heartbeat::new(heartbeat_interval),
        }))
    }
}

/// Handles close message from remote server.
///
/// This function will be called on every WebSocket close (normal and abnormal)
/// regardless of the [`CloseReason`].
fn on_close(inner_rc: &RefCell<Inner>, close_msg: &CloseMsg) {
    {
        let mut inner = inner_rc.borrow_mut();
        inner.sock.take();
        inner.heartbeat.stop();
    }

    if let CloseMsg::Normal(_, reason) = &close_msg {
        // This is reconnecting and this is not considered as connection
        // close.
        if let CloseReason::Reconnected = reason {
        } else {
            let mut on_close_by_server_subscribers = Vec::new();
            std::mem::swap(
                &mut on_close_by_server_subscribers,
                &mut inner_rc.borrow_mut().on_close_by_server_subscribers,
            );

            for sub in on_close_by_server_subscribers {
                if let Err(reason) = sub.send(reason.clone()) {
                    web_sys::console::error_1(
                        &format!(
                            "Failed to send reason of Jason close to \
                             subscriber: {:?}",
                            reason
                        )
                        .into(),
                    )
                }
            }
        }
    }

    // TODO: reconnect on disconnect, propagate error if unable
    //       to reconnect
    match close_msg {
        CloseMsg::Normal(_, _) | CloseMsg::Disconnect(_) => {}
    }
}

/// Handles messages from remote server.
fn on_message(inner_rc: &RefCell<Inner>, msg: Result<ServerMsg, WasmErr>) {
    let inner = inner_rc.borrow();
    match msg {
        Ok(ServerMsg::Pong(_num)) => {
            // TODO: detect no pings
            inner.heartbeat.set_pong_at(Date::now());
        }
        Ok(ServerMsg::Event(event)) => {
            // TODO: many subs, filter messages by session
            if let Some(sub) = inner.subs.iter().next() {
                if let Err(err) = sub.unbounded_send(event) {
                    // TODO: receiver is gone, should delete
                    //       this subs tx
                    WasmErr::from(err).log_err();
                }
            }
        }
        Err(err) => {
            // TODO: protocol versions mismatch? should drop
            //       connection if so
            err.log_err();
        }
    }
}

impl WebsocketRpcClient {
    pub fn new(ping_interval: i32) -> Self {
        Self(Inner::new(ping_interval))
    }
}

impl RpcClient for WebsocketRpcClient {
    /// Creates new WebSocket connection to remote media server.
    /// Starts `Heartbeat` if connection succeeds and binds handlers
    /// on receiving messages from server and closing socket.
    fn connect(
        &self,
        token: String,
    ) -> LocalBoxFuture<'static, Result<(), WasmErr>> {
        let inner = Rc::clone(&self.0);
        Box::pin(async move {
            let socket = Rc::new(WebSocket::new(&token).await?);
            inner.borrow_mut().heartbeat.start(Rc::clone(&socket))?;

            let inner_rc = Rc::clone(&inner);
            socket.on_message(move |msg: Result<ServerMsg, WasmErr>| {
                on_message(&inner_rc, msg)
            })?;

            let inner_rc = Rc::clone(&inner);
            socket.on_close(move |msg: CloseMsg| on_close(&inner_rc, &msg))?;

            inner.borrow_mut().sock.replace(socket);
            Ok(())
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
    /// RPC connection closing initiated by server.
    fn on_close_by_server(
        &self,
    ) -> LocalBoxFuture<'static, Result<CloseReason, oneshot::Canceled>> {
        let (tx, rx) = oneshot::channel();
        self.0.borrow_mut().on_close_by_server_subscribers.push(tx);
        Box::pin(rx)
    }
}

impl Drop for WebsocketRpcClient {
    /// Drops related connection and its [`Heartbeat`].
    fn drop(&mut self) {
        self.0.borrow_mut().sock.take();
        self.0.borrow_mut().heartbeat.stop();
    }
}

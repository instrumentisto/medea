//! Abstraction over RPC transport.

mod heartbeat;
mod websocket;

use std::{cell::RefCell, rc::Rc, vec};

use anyhow::Result;
use derive_more::Display;
use futures::{
    channel::{mpsc, oneshot},
    future::LocalBoxFuture,
    stream::LocalBoxStream,
};
use js_sys::Date;
use medea_client_api_proto::{
    ClientMsg, CloseDescription, CloseReason as CloseByServerReason, Command,
    Event, ServerMsg,
};
use web_sys::CloseEvent;

use self::{
    heartbeat::Heartbeat,
    websocket::{Error, WebSocket},
};

/// Reasons of closing by client side.
#[derive(Clone, Display, Debug)]
pub enum CloseByClientReason {
    /// [`Room`] was dropped without `close_reason`.
    RoomUnexpectedlyDropped,

    /// Room was normally closed by JS side.
    RoomClosed,

    /// [`WebSocketRpcClient`] was unexpectedly dropped.
    RpcConnectionUnexpectedlyDropped,
}

impl CloseByClientReason {
    /// Returns `true` if [`CloseByClientReason`] is considered as error.
    pub fn is_err(&self) -> bool {
        match &self {
            Self::RoomUnexpectedlyDropped
            | Self::RpcConnectionUnexpectedlyDropped => true,
            Self::RoomClosed => false,
        }
    }
}

impl Into<CloseReason> for CloseByClientReason {
    fn into(self) -> CloseReason {
        CloseReason::ByClient {
            is_err: self.is_err(),
            reason: self,
        }
    }
}

/// Reasons of closing by client side and server side.
#[derive(Clone, Display, Debug)]
pub enum CloseReason {
    /// Closed by server.
    ByServer(CloseByServerReason),

    /// Closed by client.
    #[display(fmt = "{}", reason)]
    ByClient {
        /// Reason of closing.
        reason: CloseByClientReason,

        /// Is closing considered as error.
        is_err: bool,
    },
}

/// Connection with remote was closed.
#[derive(Debug)]
pub enum CloseMsg {
    /// Transport was gracefully closed by remote.
    ///
    /// Determines by close code `1000` and existence of
    /// [`RpcConnectionCloseReason`].
    Normal(u16, CloseByServerReason),

    /// Connection was unexpectedly closed. Consider reconnecting.
    ///
    /// Unexpected close determines by close code != `1000` and for close code
    /// 1000 without reason. This is used because if connection lost then
    /// close code will be `1000` which is wrong.
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

// TODO: consider using async-trait crate, it doesnt work with mockall atm
/// Client to talk with server via Client API RPC.
#[allow(clippy::module_name_repetitions)]
#[cfg_attr(feature = "mockable", mockall::automock)]
pub trait RpcClient {
    /// Establishes connection with RPC server.
    fn connect(&self, token: String) -> LocalBoxFuture<'static, Result<()>>;

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
    fn on_close(
        &self,
    ) -> LocalBoxFuture<'static, Result<CloseReason, oneshot::Canceled>>;
}

// TODO:
// 1. Proper sub registry.
// 2. Reconnect.
// 3. Disconnect if no pongs.
// 4. Buffering if no socket?
/// Client API RPC client to talk with server via [`WebSocket`].
pub struct WebSocketRpcClient(Rc<RefCell<Inner>>);

/// Inner state of [`WebsocketRpcClient`].
struct Inner {
    /// [`WebSocket`] connection to remote media server.
    sock: Option<Rc<WebSocket>>,

    heartbeat: Heartbeat,

    /// Event's subscribers list.
    subs: Vec<mpsc::UnboundedSender<Event>>,

    /// [`oneshot::Sender`] with which [`CloseReason`] will be sent when
    /// WebSocket connection normally closed by server.
    ///
    /// Note that [`CloseReason`] will not be sent if WebSocket closed with
    /// [`RpcConnectionCloseReason::NewConnection`] reason.
    on_close_subscribers: Vec<oneshot::Sender<CloseReason>>,
}

impl Inner {
    fn new(heartbeat_interval: i32) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            sock: None,
            on_close_subscribers: Vec::new(),
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
    let mut inner = inner_rc.borrow_mut();
    inner.sock.take();
    inner.heartbeat.stop();

    if let CloseMsg::Normal(_, reason) = &close_msg {
        // This is reconnecting and this is not considered as connection
        // close.
        if let CloseByServerReason::Reconnected = reason {
        } else {
            for sub in inner.on_close_subscribers.drain(..) {
                if let Err(reason) =
                    sub.send(CloseReason::ByServer(reason.clone()))
                {
                    console_error!(format!(
                        "Failed to send reason of Jason close to subscriber: \
                         {:?}",
                        reason
                    ))
                }
            }
        }
    }

    // TODO: reconnect on disconnect, propagate error if unable
    //       to reconnect
    match close_msg {
        CloseMsg::Normal(_, _) | CloseMsg::Abnormal(_) => {}
    }
}

/// Handles messages from remote server.
fn on_message(inner_rc: &RefCell<Inner>, msg: Result<ServerMsg, Error>) {
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
                    console_error!(err.to_string());
                }
            }
        }
        Err(err) => {
            // TODO: protocol versions mismatch? should drop
            //       connection if so
            console_error!(err.to_string());
        }
    }
}

impl WebSocketRpcClient {
    /// Creates new [`WebsocketRpcClient`] with a given `ping_interval` in
    /// milliseconds.
    pub fn new(ping_interval: i32) -> Self {
        Self(Inner::new(ping_interval))
    }
}

impl RpcClient for WebSocketRpcClient {
    /// Creates new WebSocket connection to remote media server.
    /// Starts `Heartbeat` if connection succeeds and binds handlers
    /// on receiving messages from server and closing socket.
    fn connect(&self, token: String) -> LocalBoxFuture<'static, Result<()>> {
        let inner = Rc::clone(&self.0);
        Box::pin(async move {
            let socket = Rc::new(WebSocket::new(&token).await?);
            inner.borrow_mut().heartbeat.start(Rc::clone(&socket))?;

            let inner_rc = Rc::clone(&inner);
            socket.on_message(move |msg: Result<ServerMsg, Error>| {
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
    /// RPC connection closing.
    fn on_close(
        &self,
    ) -> LocalBoxFuture<'static, Result<CloseReason, oneshot::Canceled>> {
        let (tx, rx) = oneshot::channel();
        self.0.borrow_mut().on_close_subscribers.push(tx);
        Box::pin(rx)
    }
}

impl Drop for WebSocketRpcClient {
    /// Drops related connection and its [`Heartbeat`].
    fn drop(&mut self) {
        self.0.borrow_mut().sock.take();
        self.0.borrow_mut().heartbeat.stop();
    }
}

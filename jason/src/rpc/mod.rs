//! Abstraction over RPC transport.

mod heartbeat;
mod websocket;

use std::{cell::RefCell, rc::Rc, vec};

use anyhow::Result;
use derive_more::Display;
use futures::{
    channel::{mpsc, oneshot},
    future::LocalBoxFuture,
    stream::{LocalBoxStream, StreamExt as _},
};
use js_sys::Date;
use medea_client_api_proto::{
    ClientMsg, CloseDescription, CloseReason as CloseByServerReason, Command,
    Event, ServerMsg,
};
use wasm_bindgen_futures::spawn_local;
use web_sys::CloseEvent;

use self::heartbeat::Heartbeat;

#[doc(inline)]
pub use self::websocket::{Error as TransportError, WebSocketRpcTransport};

/// Reasons of closing by client side.
#[derive(Clone, Display, Debug, PartialEq, Eq)]
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
#[derive(Clone, Display, Debug, Eq, PartialEq)]
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
    fn connect(
        &self,
        transport: Rc<dyn RpcTransport>,
    ) -> LocalBoxFuture<'static, Result<()>>;

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

/// RPC transport between client and server.
#[allow(clippy::module_name_repetitions)]
#[cfg_attr(feature = "mockable", mockall::automock)]
pub trait RpcTransport {
    /// Sets handler on receiving message from server.
    fn on_message(
        &self,
    ) -> Result<
        LocalBoxStream<'static, Result<ServerMsg, TransportError>>,
        TransportError,
    >;

    /// Sets handler on closing RPC connection.
    fn on_close(
        &self,
    ) -> Result<
        LocalBoxFuture<'static, Result<CloseMsg, oneshot::Canceled>>,
        TransportError,
    >;

    /// Sends message to server.
    fn send(&self, msg: &ClientMsg) -> Result<(), TransportError>;
}

/// Inner state of [`WebsocketRpcClient`].
struct Inner {
    /// [`WebSocket`] connection to remote media server.
    sock: Option<Rc<dyn RpcTransport>>,

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
            inner
                .on_close_subscribers
                .drain(..)
                .filter_map(|sub| {
                    sub.send(CloseReason::ByServer(reason.clone())).err()
                })
                .for_each(|reason| {
                    console_error!(format!(
                        "Failed to send reason of Jason close to subscriber: \
                         {:?}",
                        reason
                    ))
                });
        }
    }

    // TODO: reconnect on disconnect, propagate error if unable
    //       to reconnect
    match close_msg {
        CloseMsg::Normal(_, _) | CloseMsg::Abnormal(_) => {}
    }
}

/// Handles messages from remote server.
fn on_message(
    inner_rc: &RefCell<Inner>,
    msg: Result<ServerMsg, TransportError>,
) {
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

// TODO:
// 1. Proper sub registry.
// 2. Reconnect.
// 3. Disconnect if no pongs.
// 4. Buffering if no socket?
/// Client API RPC client to talk with server via [`WebSocket`].
#[allow(clippy::module_name_repetitions)]
pub struct WebSocketRpcClient(Rc<RefCell<Inner>>);

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
    fn connect(
        &self,
        transport: Rc<dyn RpcTransport>,
    ) -> LocalBoxFuture<'static, Result<()>> {
        let inner = Rc::clone(&self.0);
        Box::pin(async move {
            inner.borrow_mut().heartbeat.start(Rc::clone(&transport))?;

            let inner_rc = Rc::clone(&inner);
            let mut on_socket_message = transport.on_message()?;
            spawn_local(async move {
                while let Some(msg) = on_socket_message.next().await {
                    on_message(&inner_rc, msg)
                }
            });

            let inner_rc = Rc::clone(&inner);
            let on_socket_close = transport.on_close()?;
            spawn_local(async move {
                match on_socket_close.await {
                    Ok(msg) => on_close(&inner_rc, &msg),
                    Err(e) => {
                        console_error!(format!(
                            "RPC socket was unexpectedly dropped. {:?}",
                            e
                        ));
                    }
                }
            });

            inner.borrow_mut().sock.replace(transport);
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

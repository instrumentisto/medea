//! Abstraction over RPC transport.

mod heartbeat;
mod websocket;

use std::{cell::RefCell, rc::Rc, vec};

use anyhow::Result;
use futures::{
    channel::{mpsc, oneshot},
    future::LocalBoxFuture,
    stream::{LocalBoxStream, StreamExt as _},
};
use js_sys::Date;
use medea_client_api_proto::{ClientMsg, Command, Event, ServerMsg};
use wasm_bindgen_futures::spawn_local;

use self::heartbeat::Heartbeat;

#[doc(inline)]
pub use self::websocket::{Error as TransportError, WebSocketRpcTransport};

/// Connection with remote was closed.
#[derive(Debug)]
pub enum CloseMsg {
    /// Transport was gracefully closed by remote.
    Normal(String),
    /// Connection was unexpectedly closed. Consider reconnecting.
    Disconnect(String),
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
}

impl Inner {
    fn new(heartbeat_interval: i32) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            sock: None,
            subs: vec![],
            heartbeat: Heartbeat::new(heartbeat_interval),
        }))
    }
}

/// Handles close messsage from remote server.
fn on_close(inner_rc: &RefCell<Inner>, close_msg: CloseMsg) {
    let mut inner = inner_rc.borrow_mut();
    inner.sock.take();
    inner.heartbeat.stop();

    // TODO: reconnect on disconnect, propagate error if unable
    //       to reconnect
    match close_msg {
        CloseMsg::Normal(_msg) | CloseMsg::Disconnect(_msg) => {}
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
                    Ok(msg) => on_close(&inner_rc, msg),
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
}

impl Drop for WebSocketRpcClient {
    /// Drops related connection and its [`Heartbeat`].
    fn drop(&mut self) {
        self.0.borrow_mut().sock.take();
        self.0.borrow_mut().heartbeat.stop();
    }
}

//! Abstraction over RPC transport.

mod heartbeat;
mod websocket;

use std::{cell::RefCell, rc::Rc, vec};

use futures::{channel::mpsc, future::LocalBoxFuture, stream::LocalBoxStream};
use js_sys::Date;
use medea_client_api_proto::{
    ClientMsg, CloseDescription, CloseReason, Command, Event, ServerMsg,
};
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

// TODO: consider using async-trait crate, it doesnt work with mockall atm
/// Client to talk with server via Client API RPC.
#[allow(clippy::module_name_repetitions)]
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
    #[cfg(not(feature = "mockable"))]
    fn on_close_by_server(&self, f: Box<dyn Fn(&CloseMsg)>);
}

// We mock `RpcClient` manually because `mockall` can't mock `Fn` objects but
// in spike of `#[cfg(not(feature = "mockable"))]` it still tries mock
// `on_close_room`.
//
// This macro will generate `MockRpcClient` mock for `RpcClient` which you can
// use in tests with 'mockable' feature.
//
// Note that functional of closing 'Room' on WebSocket close will not be
// available in tests with mocks because limitations of `mockall` crate.
#[cfg(feature = "mockable")]
mockall::mock! {
    pub RpcClient {}

    pub trait RpcClient {
        fn connect(
            &self,
            token: String,
        ) -> LocalBoxFuture<'static, Result<(), WasmErr>>;

        /// Returns [`Stream`] of all [`Event`]s received by this [`RpcClient`].
        fn subscribe(&self) -> LocalBoxStream<'static, Event>;

        /// Unsubscribes from this [`RpcClient`]. Drops all subscriptions atm.
        fn unsub(&self);

        /// Sends [`Command`] to server.
        fn send_command(&self, command: Command);
    }
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

    /// Closure which will be called when WebSocket connection normally closed
    /// by server.
    ///
    /// Note that this closure will not be called if WebSocket closed with
    /// [`RpcConnectionCloseReason::NewConnection`] reason.
    ///
    /// [`Rc`] needed for fix `BorrowMut` error of [`WebSocketRpcClient`] when
    /// we drop all [`Room`]s from [`Jason`].
    ///
    /// [`Room`]: crate::api::room::Room
    /// [`Jason`]: crate::api::Jason
    on_close_by_server: Rc<Box<dyn Fn(&CloseMsg)>>,
}

impl Inner {
    fn new(heartbeat_interval: i32) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            sock: None,
            on_close_by_server: Rc::new(Box::new(|_| {})),
            subs: vec![],
            heartbeat: Heartbeat::new(heartbeat_interval),
        }))
    }
}

/// Handles close message from remote server.
///
/// This function will be called on every WebSocket close (normal and abnormal)
/// regardless of the close reason.
fn on_close(inner_rc: &RefCell<Inner>, close_msg: &CloseMsg) {
    {
        let mut inner = inner_rc.borrow_mut();
        inner.sock.take();
        inner.heartbeat.stop();
    }

    if let CloseMsg::Normal(_, reason) = &close_msg {
        match reason {
            // This is reconnecting and this is not considered as connection
            // close.
            CloseReason::Reconnected => {}
            _ => {
                let f = Rc::clone(&inner_rc.borrow().on_close_by_server);
                (f)(&close_msg);
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

    // Not available in mockable tests because limitations of
    // `mockall`.
    #[cfg(not(feature = "mockable"))]
    fn on_close_by_server(&self, f: Box<dyn Fn(&CloseMsg)>) {
        self.0.borrow_mut().on_close_by_server = Rc::new(f);
    }
}

impl Drop for WebsocketRpcClient {
    /// Drops related connection and its [`Heartbeat`].
    fn drop(&mut self) {
        self.0.borrow_mut().sock.take();
        self.0.borrow_mut().heartbeat.stop();
    }
}

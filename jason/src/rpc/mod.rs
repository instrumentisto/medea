//! Abstraction over RPC transport.

mod heartbeat;
mod websocket;

use std::{cell::RefCell, rc::Rc, vec};

use futures::{
    sync::mpsc::{unbounded, UnboundedSender},
    Future, Stream,
};
use js_sys::Date;
use medea_client_api_proto::{ClientMsg, Command, Event, ServerMsg};

use crate::utils::WasmErr;

use self::{heartbeat::Heartbeat, websocket::WebSocket};

/// Connection with remote was closed.
pub enum CloseMsg {
    /// Transport was gracefully closed by remote.
    Normal(String),
    /// Connection was unexpectedly closed. Consider reconnecting.
    Disconnect(String),
}

// TODO:
// 1. Proper sub registry.
// 2. Reconnect.
// 3. Disconnect if no pongs.
// 4. Buffering if no socket?
#[allow(clippy::module_name_repetitions)]
pub struct RpcClient(Rc<RefCell<Inner>>);

/// Inner state of [`RpcClient`].
struct Inner {
    /// WebSocket connection to remote media server.
    sock: Option<Rc<WebSocket>>,

    /// Credentials used to authorize connection.
    token: String,

    heartbeat: Heartbeat,

    /// Event's subscribers list.
    subs: Vec<UnboundedSender<Event>>,
}

impl Inner {
    fn new(token: String, heartbeat_interval: i32) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            sock: None,
            token,
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

impl RpcClient {
    pub fn new(token: String, ping_interval: i32) -> Self {
        Self(Inner::new(token, ping_interval))
    }

    /// Creates new WebSocket connection to remote media server.
    /// Starts [`Heatbeat`] if connection succeeds and binds handlers
    /// on receiving messages from server and closing socket.
    pub fn init(&mut self) -> impl Future<Item = (), Error = WasmErr> {
        let inner = Rc::clone(&self.0);
        WebSocket::new(&self.0.borrow().token).and_then(
            move |socket: WebSocket| {
                let socket = Rc::new(socket);

                inner.borrow_mut().heartbeat.start(Rc::clone(&socket))?;

                let inner_rc = Rc::clone(&inner);
                socket.on_message(move |msg: Result<ServerMsg, WasmErr>| {
                    on_message(&inner_rc, msg)
                })?;

                let inner_rc = Rc::clone(&inner);
                socket
                    .on_close(move |msg: CloseMsg| on_close(&inner_rc, msg))?;

                inner.borrow_mut().sock.replace(socket);
                Ok(())
            },
        )
    }

    /// Returns Stream of all Events received by this [`RpcClient`].
    // TODO: proper sub registry
    pub fn subscribe(&self) -> impl Stream<Item = Event, Error = ()> {
        let (tx, rx) = unbounded();
        self.0.borrow_mut().subs.push(tx);

        rx
    }

    /// Unsubscribe from this [`RpcClient`]. Drops all subscriptions atm.
    // TODO: proper sub registry
    pub fn unsub(&self) {
        self.0.borrow_mut().subs.clear();
    }

    /// Sends Command to Medea.
    // TODO: proper sub registry
    pub fn send_command(&self, command: Command) {
        let socket_borrow = &self.0.borrow().sock;

        // TODO: no socket? we dont really want this method to return err
        if let Some(socket) = socket_borrow.as_ref() {
            socket.send(&ClientMsg::Command(command)).unwrap();
        }
    }
}

impl Drop for RpcClient {
    /// Drops related connection and its [`Heartbeat`].
    fn drop(&mut self) {
        self.0.borrow_mut().sock.take();
        self.0.borrow_mut().heartbeat.stop();
    }
}

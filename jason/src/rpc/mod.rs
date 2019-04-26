//!  Abstraction over concrete transport.

mod pinger;
mod websocket;

pub mod protocol;

use futures::{
    sync::mpsc::{unbounded, UnboundedSender},
    Future, Stream,
};
use js_sys::Date;

use std::{cell::RefCell, rc::Rc, vec};

use crate::{
    rpc::protocol::{ClientMsg, ServerMsg},
    utils::WasmErr,
};

use self::{
    pinger::Pinger,
    protocol::{Command, Event},
    websocket::WebSocket,
};

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
pub struct RPCClient(Rc<RefCell<Inner>>);

struct Inner {
    sock: Option<Rc<WebSocket>>,
    token: String,
    pinger: Pinger,
    subs: Vec<UnboundedSender<Event>>,
}

impl Inner {
    fn new(token: String, ping_interval: i32) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            sock: None,
            token,
            subs: vec![],
            pinger: Pinger::new(ping_interval),
        }))
    }
}

impl RPCClient {
    pub fn new(token: String, ping_interval: i32) -> Self {
        Self(Inner::new(token, ping_interval))
    }

    pub fn init(&mut self) -> impl Future<Item = (), Error = WasmErr> {
        let inner = Rc::clone(&self.0);
        WebSocket::new(&self.0.borrow().token).and_then(
            move |socket: WebSocket| {
                let socket = Rc::new(socket);
                inner.borrow_mut().pinger.start(Rc::clone(&socket))?;

                let inner_rc = Rc::clone(&inner);
                socket.on_message(move |msg: Result<ServerMsg, WasmErr>| {
                    let inner = inner_rc.borrow();
                    match msg {
                        Ok(ServerMsg::Pong(_num)) => {
                            // TODO: detect no pings
                            inner.pinger.set_pong_at(Date::now());
                        }
                        Ok(ServerMsg::Event(event)) => {
                            // TODO: many subs, filter messages by session
                            if let Some(sub) = inner.subs.iter().next() {
                                if let Err(err) = sub.unbounded_send(event) {
                                    // TODO: receiver is gone, should delete
                                    // this
                                    // subs tx
                                    WasmErr::from(err).log_err();
                                }
                            }
                        }
                        Err(err) => {
                            // TODO: protocol versions mismatch? should drop
                            // connection if so
                            err.log_err();
                        }
                    }
                })?;

                let inner_rc = Rc::clone(&inner);
                socket.on_close(move |msg: CloseMsg| {
                    let mut inner = inner_rc.borrow_mut();
                    inner.sock.take();
                    inner.pinger.stop();

                    // TODO: reconnect on disconnect, propagate error if unable
                    // to reconnect
                    match msg {
                        CloseMsg::Normal(_msg) | CloseMsg::Disconnect(_msg) => {
                        }
                    }
                })?;

                inner.borrow_mut().sock.replace(socket);
                Ok(())
            },
        )
    }

    // TODO: proper sub registry
    pub fn subscribe(&self) -> impl Stream<Item = Event, Error = ()> {
        let (tx, rx) = unbounded();
        self.0.borrow_mut().subs.push(tx);

        rx
    }

    // TODO: proper sub registry
    pub fn unsub(&self) {
        self.0.borrow_mut().subs.clear();
    }

    // TODO: proper sub registry
    pub fn _send_command(&self, command: Command) {
        let socket_borrow = &self.0.borrow().sock;

        // TODO: no socket? we dont really want this method to return err
        if let Some(socket) = socket_borrow.as_ref() {
            socket.send(&ClientMsg::Command(command)).unwrap();
        }
    }
}

impl Drop for RPCClient {
    fn drop(&mut self) {
        // Drop socket, pinger will be dropped too
        self.0.borrow_mut().sock.take();
    }
}

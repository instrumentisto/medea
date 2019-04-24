//!  Abstraction over concrete transport.
//!
mod pinger;
mod websocket;

pub mod protocol;

use futures::sync::mpsc::UnboundedSender;
use js_sys::Date;

use std::{cell::RefCell, rc::Rc, vec};

use crate::{
    rpc::protocol::{InMsg, OutMsg},
    utils::WasmErr,
};

use self::{
    pinger::Pinger,
    protocol::{Command, Event as MedeaEvent},
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
pub struct RPCClient {
    sock: Rc<RefCell<Option<WebSocket>>>,
    token: String,
    pinger: Rc<Pinger>,
    subs: Rc<RefCell<Vec<UnboundedSender<MedeaEvent>>>>,
}

impl RPCClient {
    pub fn new(token: String, ping_interval: i32) -> Self {
        Self {
            sock: Rc::new(RefCell::new(None)),
            token,
            subs: Rc::new(RefCell::new(vec![])),
            pinger: Rc::new(Pinger::new(ping_interval)),
        }
    }

    pub fn init(&mut self) -> Result<(), WasmErr> {
        self.sock = Rc::new(RefCell::new(Some(WebSocket::new(&self.token)?)));

        let socket_borrow = self.sock.borrow();
        let socket_ref = socket_borrow
            .as_ref()
            .ok_or_else(|| WasmErr::from_str("socket is None"))?;

        let socket = Rc::clone(&self.sock);
        let pinger = Rc::clone(&self.pinger);
        socket_ref.on_open(move || {
            if let Err(err) = pinger.start(socket) {
                err.log_err();
            };
        })?;

        let pinger = Rc::clone(&self.pinger);
        let subs = Rc::clone(&self.subs);
        socket_ref.on_message(move |msg: Result<InMsg, WasmErr>| {
            match msg {
                Ok(InMsg::Pong(_num)) => {
                    // TODO: detect no pings
                    pinger.set_pong_at(Date::now());
                }
                Ok(InMsg::Event(event)) => {
                    // TODO: many subs, filter messages by session
                    let subs_borrow = subs.borrow();

                    if let Some(sub) = subs_borrow.iter().next() {
                        if let Err(err) = sub.unbounded_send(event) {
                            // TODO: receiver is gone, should delete this
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

        let socket = Rc::clone(&self.sock);
        let pinger = Rc::clone(&self.pinger);
        socket_ref.on_close(move |msg: CloseMsg| {
            socket.borrow_mut().take();
            pinger.stop();

            // TODO: reconnect on disconnect, propagate error if unable
            // to reconnect
            match msg {
                CloseMsg::Normal(_msg) | CloseMsg::Disconnect(_msg) => {}
            }
        })?;

        Ok(())
    }

    // TODO: proper sub registry
    pub fn add_sub(&self, sub: UnboundedSender<MedeaEvent>) {
        self.subs.borrow_mut().push(sub);
    }

    // TODO: proper sub registry
    pub fn unsub(&self) {
        self.subs.borrow_mut().clear();
    }

    // TODO: proper sub registry
    pub fn _send_command(&self, command: Command) {
        let socket_borrow = self.sock.borrow();

        // TODO: no socket? we dont really want this method to return err
        if let Some(socket) = socket_borrow.as_ref() {
            socket.send(&OutMsg::Command(command)).unwrap();
        }
    }
}

impl Drop for RPCClient {
    fn drop(&mut self) {
        // Drop socket, pinger will be dropped too
        self.sock.borrow_mut().take();
    }
}

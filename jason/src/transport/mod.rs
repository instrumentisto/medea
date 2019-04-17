mod pinger;
mod websocket;

pub mod protocol;

use futures::sync::mpsc::UnboundedSender;
use js_sys::Date;

use std::{cell::RefCell, rc::Rc, vec};

use crate::{
    transport::protocol::{InMsg, OutMsg},
    utils::WasmErr,
};

use self::{
    pinger::Pinger,
    protocol::{Command, Event as MedeaEvent},
    websocket::WebSocket,
};

// TODO:
// 1. Reconnect.
// 2. Disconnect if no pongs.
pub struct Transport {
    sock: Rc<RefCell<Option<WebSocket>>>,
    token: String,
    pinger: Rc<Pinger>,
    subs: Rc<RefCell<Vec<UnboundedSender<MedeaEvent>>>>,
}

impl Transport {
    pub fn new(token: String, ping_interval: i32) -> Self {
        Self {
            sock: Rc::new(RefCell::new(None)),
            token,
            subs: Rc::new(RefCell::new(vec![])),
            pinger: Rc::new(Pinger::new(ping_interval)),
        }
    }

    pub fn init(&mut self) -> Result<(), WasmErr> {
        let socket = WebSocket::new(&self.token)?;
        let socket = Rc::new(RefCell::new(Some(socket)));
        let socket_borrow = socket.borrow();
        let socket_ref = socket_borrow
            .as_ref()
            .ok_or_else(|| WasmErr::from_str("socket is None"))?;

        let socket_rc = Rc::clone(&socket);
        let pinger_rc: Rc<Pinger> = Rc::clone(&self.pinger);

        socket_ref.on_open(move || {
            if let Err(err) = pinger_rc.start(socket_rc) {
                err.log_err();
            };
        })?;

        let pinger_rc = Rc::clone(&self.pinger);
        let subs_rc = Rc::clone(&self.subs);
        socket_ref.on_message(move |msg: Result<InMsg, WasmErr>| {
            match msg {
                Ok(InMsg::Pong(_num)) => {
                    // TODO: detect no pings
                    pinger_rc.set_pong_at(Date::now());
                }
                Ok(InMsg::Event(event)) => {
                    // TODO: many subs, filter messages by session
                    let subs_borrow = subs_rc.borrow();
                    let sub = subs_borrow.iter().next().unwrap();

                    if let Err(err) = sub.unbounded_send(event) {
                        WasmErr::from(err).log_err();
                    }
                }
                Err(err) => {
                    // TODO: protocol versions mismatch? should drop
                    // connection if so
                    err.log_err();
                }
            }
        })?;

        let socket_rc = Rc::clone(&socket);
        let pinger_rc: Rc<Pinger> = Rc::clone(&self.pinger);
        socket_ref.on_close(move |msg: CloseMsg| {
            socket_rc.borrow_mut().take();
            pinger_rc.stop();

            // TODO: reconnect on disconnect
            match msg {
                CloseMsg::Normal(_msg) | CloseMsg::Disconnect(_msg) => {}
            }
        })?;

        drop(socket_borrow);
        self.sock = socket;

        Ok(())
    }

    pub fn add_sub(&self, sub: UnboundedSender<MedeaEvent>) {
        self.subs.borrow_mut().push(sub);
    }

    pub fn _send_command(&self, command: Command) {
        let socket_borrow = self.sock.borrow();

        // TODO: no socket? we dont really want this method to return err
        if let Some(socket) = socket_borrow.as_ref() {
            socket.send(&OutMsg::Command(command)).unwrap();
        }
    }
}

pub enum CloseMsg {
    Normal(String),
    Disconnect(String),
}

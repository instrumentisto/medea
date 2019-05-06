use protocol::ClientMsg;
use wasm_bindgen::{prelude::*, JsCast};

use std::{cell::RefCell, rc::Rc};

use crate::{
    rpc::websocket::WebSocket,
    utils::{window, IntervalHandle, WasmErr},
};

pub struct Pinger(Rc<RefCell<InnerPinger>>);

struct InnerPinger {
    ping_interval: i32,
    num: u64,
    pong_at: Option<f64>,
    socket: Option<Rc<WebSocket>>,
    ping_task: Option<PingTaskHandler>,
}

impl InnerPinger {
    fn send_now(&mut self) -> Result<(), WasmErr> {
        match self.socket.as_ref() {
            None => Err(WasmErr::from_str("Unable to ping: no socket")),
            Some(socket) => {
                self.num += 1;
                socket.send(&ClientMsg::Ping(self.num))
            }
        }
    }
}

struct PingTaskHandler {
    _ping_closure: Closure<dyn FnMut()>,
    _interval_handler: IntervalHandle,
}

impl Pinger {
    pub fn new(ping_interval: i32) -> Self {
        Self(Rc::new(RefCell::new(InnerPinger {
            ping_interval,
            num: 0,
            pong_at: None,
            socket: None,
            ping_task: None,
        })))
    }

    pub fn start(&self, socket: Rc<WebSocket>) -> Result<(), WasmErr> {
        let mut inner = self.0.borrow_mut();
        inner.socket = Some(socket);
        inner.send_now()?;

        let inner_rc = Rc::clone(&self.0);
        let do_ping = Closure::wrap(Box::new(move || {
            // its_ok if ping fails few times
            inner_rc.borrow_mut().send_now().is_ok();
        }) as Box<dyn FnMut()>);

        let interval_id = window()
            .set_interval_with_callback_and_timeout_and_arguments_0(
                do_ping.as_ref().unchecked_ref(),
                inner.ping_interval,
            )?;

        inner.ping_task = Some(PingTaskHandler {
            _ping_closure: do_ping,
            _interval_handler: IntervalHandle(interval_id),
        });

        Ok(())
    }

    pub fn set_pong_at(&self, at: f64) {
        self.0.borrow_mut().pong_at = Some(at);
    }

    pub fn stop(&self) {
        self.0.borrow_mut().ping_task.take();
    }
}

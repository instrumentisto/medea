use protocol::ClientMsg;
use wasm_bindgen::{prelude::*, JsCast};

use std::{cell::RefCell, rc::Rc};

use crate::{
    rpc::websocket::WebSocket,
    utils::{window, IntervalHandle, WasmErr},
};

/// Pinger for periodical tests connection to server by sends ping message.
pub struct Pinger(Rc<RefCell<InnerPinger>>);

struct InnerPinger {
    /// Interval for send ping message.
    ping_interval: i32,

    /// Count of ping message sending to server.
    num: u64,

    /// Count of pong message received from server.
    pong_at: Option<f64>,

    /// Socket to server.
    socket: Option<Rc<WebSocket>>,

    /// Handler for bind closure what run when ping send.
    ping_task: Option<PingTaskHandler>,
}

impl InnerPinger {
    /// Send ping message into socket.
    /// Returns error no open socket.
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

/// Handler for bind closure what run when ping send.
struct PingTaskHandler {
    _ping_closure: Closure<dyn FnMut()>,
    _interval_handler: IntervalHandle,
}

impl Pinger {
    /// Returns new instance of [`Pinger`] with given interval for ping in
    /// seconds.
    pub fn new(ping_interval: i32) -> Self {
        Self(Rc::new(RefCell::new(InnerPinger {
            ping_interval,
            num: 0,
            pong_at: None,
            socket: None,
            ping_task: None,
        })))
    }

    /// Start [`Pinger`] for give socket.
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

    /// Store number of pong message.
    pub fn set_pong_at(&self, at: f64) {
        self.0.borrow_mut().pong_at = Some(at);
    }

    /// Stop [`Pinger`].
    pub fn stop(&self) {
        self.0.borrow_mut().ping_task.take();
    }
}

use protocol::ClientMsg;
use wasm_bindgen::{prelude::*, JsCast};

use std::{cell::RefCell, rc::Rc};

use crate::{
    rpc::websocket::WebSocket,
    utils::{window, IntervalHandle, WasmErr},
};

// TODO: Implement connection loss deteection.
/// Responsible for sending/handling keep-alive requests, detecting connection
/// loss.
pub struct Pinger(Rc<RefCell<InnerPinger>>);

struct InnerPinger {
    ping_interval: i32,
    /// Sent pings counter.
    num: u64,
    /// Timestamp of last pong received.
    pong_at: Option<f64>,
    /// WebSocket connection with remote server.
    socket: Option<Rc<WebSocket>>,
    /// Ping send task  handler. Task will be droped if you drop handler.
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

    /// Start [`Pinger`] for given [`WebSocket`]. Sends first ping immediately,
    /// so provided [`WebSocket`] must be active.
    pub fn start(&self, socket: Rc<WebSocket>) -> Result<(), WasmErr> {
        let mut inner = self.0.borrow_mut();
        inner.num = 0;
        inner.pong_at = None;
        inner.socket = Some(socket);
        inner.send_now()?;

        let inner_rc = Rc::clone(&self.0);
        let do_ping = Closure::wrap(Box::new(move || {
            // its_ok if ping fails few times
            let _ = inner_rc.borrow_mut().send_now();
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

    /// Stops [`Pinger`].
    pub fn stop(&self) {
        self.0.borrow_mut().ping_task.take();
        self.0.borrow_mut().socket.take();
    }

    /// Timestamp of last pong received.
    pub fn set_pong_at(&self, at: f64) {
        self.0.borrow_mut().pong_at = Some(at);
    }
}

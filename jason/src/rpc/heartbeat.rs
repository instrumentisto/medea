use std::{cell::RefCell, rc::Rc};

use derive_more::From;
use failure::Fail;
use medea_client_api_proto::ClientMsg;
use tracerr::Traced;
use wasm_bindgen::{prelude::*, JsCast};

use crate::{
    rpc::websocket::{Error as SocketError, WebSocket},
    utils::{window, IntervalHandle, WasmErr},
};

/// Describes errors that may occur into [`Heartbeat`].
#[derive(Debug, Fail, From)]
pub enum Error {
    #[fail(display = "unable to ping: no socket")]
    NoSocket,
    #[fail(display = "cannot set callback for ping send: {}", 0)]
    SetIntervalHandler(#[fail(cause)] WasmErr),
    #[fail(display = "failed send ping: {}", 0)]
    SendPing(#[fail(cause)] SocketError),
}

type Result<T, E = Traced<Error>> = std::result::Result<T, E>;

/// Responsible for sending/handling keep-alive requests, detecting connection
/// loss.
// TODO: Implement connection loss detection.
pub struct Heartbeat(Rc<RefCell<InnerHeartbeat>>);

struct InnerHeartbeat {
    interval: i32,
    /// Sent pings counter.
    num: u64,
    /// Timestamp of last pong received.
    pong_at: Option<f64>,
    /// WebSocket connection with remote server.
    socket: Option<Rc<WebSocket>>,
    /// Handler of sending `ping` task. Task is dropped if you drop handler.
    ping_task: Option<PingTaskHandler>,
}

impl InnerHeartbeat {
    /// Send ping message into socket.
    /// Returns error no open socket.
    fn send_now(&mut self) -> Result<()> {
        match self.socket.as_ref() {
            None => Err(tracerr::new!(Error::NoSocket)),
            Some(socket) => {
                self.num += 1;
                Ok(socket
                    .send(&ClientMsg::Ping(self.num))
                    .map_err(tracerr::map_from_and_wrap!())?)
            }
        }
    }
}

/// Handler for binding closure that runs when `ping` is sent.
struct PingTaskHandler {
    _closure: Closure<dyn FnMut()>,
    _interval_handler: IntervalHandle,
}

impl Heartbeat {
    /// Returns new instance of [`interval`] with given interval for ping in
    /// seconds.
    pub fn new(interval: i32) -> Self {
        Self(Rc::new(RefCell::new(InnerHeartbeat {
            interval,
            num: 0,
            pong_at: None,
            socket: None,
            ping_task: None,
        })))
    }

    /// Starts [`Heartbeat`] for given [`WebSocket`].
    ///
    /// Sends first `ping` immediately, so provided [`WebSocket`] must be
    /// active.
    pub fn start(&self, socket: Rc<WebSocket>) -> Result<()> {
        let mut inner = self.0.borrow_mut();
        inner.num = 0;
        inner.pong_at = None;
        inner.socket = Some(socket);
        inner.send_now().map_err(tracerr::wrap!())?;

        let inner_rc = Rc::clone(&self.0);
        let do_ping = Closure::wrap(Box::new(move || {
            // its_ok if ping fails few times
            let _ = inner_rc.borrow_mut().send_now();
        }) as Box<dyn FnMut()>);

        let interval_id = window()
            .set_interval_with_callback_and_timeout_and_arguments_0(
                do_ping.as_ref().unchecked_ref(),
                inner.interval,
            )
            .map_err(WasmErr::from)
            .map_err(tracerr::from_and_wrap!())?;

        inner.ping_task = Some(PingTaskHandler {
            _closure: do_ping,
            _interval_handler: IntervalHandle(interval_id),
        });

        Ok(())
    }

    /// Stops [`Heartbeat`].
    pub fn stop(&self) {
        self.0.borrow_mut().ping_task.take();
        self.0.borrow_mut().socket.take();
    }

    /// Timestamp of last pong received.
    pub fn set_pong_at(&self, at: f64) {
        self.0.borrow_mut().pong_at = Some(at);
    }
}

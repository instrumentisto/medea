use std::{cell::RefCell, rc::Rc};

use derive_more::{Display, From};
use medea_client_api_proto::ClientMsg;
use tracerr::Traced;
use wasm_bindgen::{prelude::*, JsCast};

use crate::{
    rpc::{RpcTransport, TransportError},
    utils::{window, IntervalHandle, JsCaused, JsError},
};

/// Errors that may occur in [`Heartbeat`].
#[derive(Debug, Display, From, JsCaused)]
pub enum HeartbeatError {
    /// Occurs when `ping` cannot be send because no transport.
    #[display(fmt = "unable to ping: no transport")]
    NoSocket,

    /// Occurs when a handler cannot be set to send `ping`.
    #[display(fmt = "cannot set callback for ping send: {}", _0)]
    SetIntervalHandler(JsError),

    /// Occurs when socket failed to send `ping`.
    #[display(fmt = "failed to send ping: {}", _0)]
    SendPing(#[js(cause)] TransportError),
}

type Result<T> = std::result::Result<T, Traced<HeartbeatError>>;

/// Responsible for sending/handling keep-alive requests, detecting connection
/// loss.
// TODO: Implement connection loss detection.
pub struct Heartbeat(Rc<RefCell<InnerHeartbeat>>);

struct InnerHeartbeat {
    interval: i32,
    /// Sent pings counter.
    num: u64,
    /// Timestamp of last pong received.
    pong_at: Option<u128>,
    /// Connection with remote RPC server.
    transport: Option<Rc<dyn RpcTransport>>,
    /// Handler of sending `ping` task. Task is dropped if you drop handler.
    ping_task: Option<PingTaskHandler>,
}

impl InnerHeartbeat {
    /// Send ping message to RPC server.
    /// Returns errors if no open transport found.
    fn send_now(&mut self) -> Result<()> {
        match self.transport.as_ref() {
            None => Err(tracerr::new!(HeartbeatError::NoSocket)),
            Some(transport) => {
                self.num += 1;
                Ok(transport
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
    /// milliseconds.
    pub fn new(interval: i32) -> Self {
        Self(Rc::new(RefCell::new(InnerHeartbeat {
            interval,
            num: 0,
            pong_at: None,
            transport: None,
            ping_task: None,
        })))
    }

    /// Starts [`Heartbeat`] for given [`RpcTransport`].
    ///
    /// Sends first `ping` immediately, so provided [`RpcTransport`] must be
    /// active.
    pub fn start(&self, transport: Rc<dyn RpcTransport>) -> Result<()> {
        let mut inner = self.0.borrow_mut();
        inner.num = 0;
        inner.pong_at = None;
        inner.transport = Some(transport);
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
            .map_err(JsError::from)
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
        self.0.borrow_mut().transport.take();
    }

    /// Timestamp of last pong received.
    pub fn set_pong_at(&self, at: u128) {
        self.0.borrow_mut().pong_at = Some(at);
    }

    pub fn get_pong_at(&self) -> Option<u128> {
        self.0.borrow().pong_at
    }
}

pub mod protocol;

use futures::sync::mpsc::UnboundedSender;
use js_sys::Date;
use wasm_bindgen::{prelude::*, JsCast};
use web_sys::{console, CloseEvent, Event, MessageEvent, WebSocket};

use std::{cell::RefCell, convert::TryFrom, rc::Rc, vec};

use crate::utils::{
    bind_handler_fn_mut, bind_handler_fn_once, window, IntervalHandle, WasmErr,
};

use self::protocol::{Command, Event as MedeaEvent, Heartbeat};

// TODO:
// 1. Reconnect.
// 2. Disconnect if no pongs.
pub struct Transport {
    sock: Rc<RefCell<Option<WebSocket>>>,
    token: String,
    pinger: Rc<Pinger>,
    subs: Rc<RefCell<Vec<UnboundedSender<MedeaEvent>>>>,
    on_open: Option<Closure<dyn FnMut(Event)>>,
    on_message: Option<Closure<dyn FnMut(MessageEvent)>>,
    on_close: Option<Closure<dyn FnMut(CloseEvent)>>,
}

impl Transport {
    pub fn new(token: String, ping_interval: i32) -> Self {
        Transport {
            sock: Rc::new(RefCell::new(None)),
            token,
            subs: Rc::new(RefCell::new(vec![])),
            pinger: Rc::new(Pinger::new(ping_interval)),
            on_open: None,
            on_message: None,
            on_close: None,
        }
    }

    pub fn init(&mut self) -> Result<(), WasmErr> {
        let socket = WebSocket::new(&self.token)?;
        let socket = Rc::new(RefCell::new(Some(socket)));
        let socket_borrow = socket.borrow();
        let socket_ref = socket_borrow
            .as_ref()
            .ok_or(WasmErr::from_str("socket is None"))?;

        let socket_rc = Rc::clone(&socket);
        let pinger_rc: Rc<Pinger> = Rc::clone(&self.pinger);
        let on_open =
            bind_handler_fn_once("open", socket_ref, move |_: Event| {
                if let Err(err) = pinger_rc.start(socket_rc) {
                    err.log_err();
                };
            })?;

        let pinger_rc = Rc::clone(&self.pinger);
        let subs_rc = Rc::clone(&self.subs);
        let on_message = bind_handler_fn_mut(
            "message",
            socket_ref,
            move |message: MessageEvent| {
                if let Ok(Heartbeat::Pong(num)) = Heartbeat::try_from(&message)
                {
                    pinger_rc.set_pong_at(Date::now());
                } else {
                    match MedeaEvent::try_from(&message) {
                        Ok(event) => {
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
                }
            },
        )?;

        let socket_rc = Rc::clone(&socket);
        let pinger_rc: Rc<Pinger> = Rc::clone(&self.pinger);
        let on_close = bind_handler_fn_once(
            "close",
            socket_ref,
            move |close: CloseEvent| {
                console::log_1(&JsValue::from_str(&format!(
                    "Close [code: {}, reason: {}]",
                    close.code(),
                    close.reason()
                )));
                socket_rc.borrow_mut().take();
                pinger_rc.stop();
            },
        )?;

        self.on_open = Some(on_open);
        self.on_message = Some(on_message);
        self.on_close = Some(on_close);
        drop(socket_borrow);
        self.sock = socket;

        Ok(())
    }

    pub fn add_sub(&self, sub: UnboundedSender<MedeaEvent>) {
        self.subs.borrow_mut().push(sub);
    }

    pub fn send_command(&self, command: &Command) {
        let socket_borrow = self.sock.borrow();

        // TODO: no socket? we dont really want this method to return err
        if let Some(socket) = socket_borrow.as_ref() {
            let msg = serde_json::to_string(&command).unwrap();

            socket.send_with_str(&msg).unwrap();
        }
    }
}

impl Drop for Transport {
    fn drop(&mut self) {
        let socket_borrow = self.sock.borrow();
        if let Some(socket) = socket_borrow.as_ref() {
            socket
                .close_with_code_and_reason(1001, "Dropped suddenly")
                .is_ok();
        }
    }
}

struct Pinger(Rc<RefCell<InnerPinger>>);

struct InnerPinger {
    ping_interval: i32,
    num: usize,
    pong_at: Option<f64>,
    socket: Rc<RefCell<Option<WebSocket>>>,
    ping_task: Option<PingTaskHandler>,
}

impl InnerPinger {
    fn send_now(&mut self) -> Result<(), WasmErr> {
        let borrow = self.socket.try_borrow()?;
        let socket = borrow
            .as_ref()
            .ok_or(WasmErr::from_str("Unable to ping: no socket"))?;
        self.num += 1;
        let msg = serde_json::to_string(&Heartbeat::Ping(self.num))?;
        socket.send_with_str(&msg).map_err(|e| WasmErr::from(e))
    }
}

struct PingTaskHandler {
    _ping_closure: Closure<dyn FnMut()>,
    _interval_handler: IntervalHandle,
}

impl Pinger {
    fn new(ping_interval: i32) -> Self {
        Self(Rc::new(RefCell::new(InnerPinger {
            ping_interval,
            num: 0,
            pong_at: None,
            socket: Rc::new(RefCell::new(None)),
            ping_task: None,
        })))
    }

    fn set_pong_at(&self, at: f64) {
        self.0.borrow_mut().pong_at = Some(at);
    }

    fn start(
        &self,
        socket: Rc<RefCell<Option<WebSocket>>>,
    ) -> Result<(), WasmErr> {
        let mut inner = self.0.borrow_mut();
        inner.socket = socket;
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

    fn stop(&self) {
        self.0.borrow_mut().ping_task.take();
    }
}

fn js_val_to_string(msg: &MessageEvent) -> Result<String, WasmErr> {
    let payload = msg.data();
    payload
        .as_string()
        .ok_or(WasmErr::from_str("Payload is not string"))
}

impl TryFrom<&MessageEvent> for MedeaEvent {
    type Error = WasmErr;

    fn try_from(msg: &MessageEvent) -> Result<Self, Self::Error> {
        serde_json::from_str::<MedeaEvent>(&js_val_to_string(msg)?)
            .map_err(|e: serde_json::error::Error| WasmErr::from(e))
    }
}

impl TryFrom<&MessageEvent> for Heartbeat {
    type Error = WasmErr;

    fn try_from(msg: &MessageEvent) -> Result<Self, Self::Error> {
        serde_json::from_str::<Heartbeat>(&js_val_to_string(msg)?)
            .map_err(|e: serde_json::error::Error| WasmErr::from(e))
    }
}

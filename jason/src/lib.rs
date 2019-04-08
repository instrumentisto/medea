use futures::sync::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use js_sys::{Date};
use serde::{Deserialize, Serialize};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{console, MessageEvent, WebSocket, Event, CloseEvent};

mod utils;

use utils::{window, bind_handler_fn_mut, bind_handler_fn_once, IntervalHandle};

// When the `console_error_panic_hook` feature is enabled, we can call the
// `set_panic_hook` function at least once during initialization, and then
// we will get better error messages if our code ever panics.
//
// For more details see
// https://github.com/rustwasm/console_error_panic_hook#readme
#[cfg(feature = "console_error_panic_hook")]
pub use console_error_panic_hook::set_once as set_panic_hook;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

//TODO:
//1. Reconnect.
//2. Disconnect if no pongs.
struct Transport {
    sock: Rc<RefCell<Option<WebSocket>>>,
    token: String,
    pinger: Rc<Pinger>,
    subs: Vec<UnboundedSender<Command>>,
    on_open: Option<Closure<dyn FnMut(Event)>>,
    on_message: Option<Closure<dyn FnMut(MessageEvent)>>,
    on_close: Option<Closure<dyn FnMut(CloseEvent)>>,
}

impl Transport {
    fn new(token: String) -> Self {
        Transport {
            sock: Rc::new(RefCell::new(None)),
            token,
            subs: vec![],
            pinger: Rc::new(Pinger::new()),
            on_open: None,
            on_message: None,
            on_close: None
        }
    }

    fn init(&mut self) {
        let socket = WebSocket::new(&self.token).unwrap();
        let socket = Rc::new(RefCell::new(Some(socket)));
        let socket_borrow = socket.borrow();
        let socket_ref = socket_borrow.as_ref().unwrap();

        let socket_rc = Rc::clone(&socket);
        let pinger_rc: Rc<Pinger> = Rc::clone(&self.pinger);
        let on_open = bind_handler_fn_once("open", socket_ref, move |_: Event| {
            pinger_rc.start(socket_rc);
        }).unwrap();

        let pinger_rc = Rc::clone(&self.pinger);
        let on_message = bind_handler_fn_mut("message", socket_ref, move |event: MessageEvent| {
            let payload = event.data();

            pinger_rc.set_pong_at(Date::now());

            console::log(&js_sys::Array::from(&payload));
        }).unwrap();

        let socket_rc = Rc::clone(&socket);
        let pinger_rc: Rc<Pinger> = Rc::clone(&self.pinger);
        let on_close = bind_handler_fn_once("close", socket_ref, move |close: CloseEvent| {
            console::log(&js_sys::Array::from(&JsValue::from_str(&format!("Close [code: {}, reason: {}]", close.code(), close.reason()))));
            socket_rc.borrow_mut().take();
            pinger_rc.stop();

        }).unwrap();

        self.on_open = Some(on_open);
        self.on_message = Some(on_message);
        self.on_close = Some(on_close);
        drop(socket_borrow);
        self.sock = socket
    }

    fn add_sub(&mut self, sub: UnboundedSender<Command>) {
        self.subs.push(sub);
    }
}

struct Pinger(Rc<RefCell<InnerPinger>>);

struct InnerPinger {
    num: usize,
    pong_at: Option<f64>,
    socket: Rc<RefCell<Option<WebSocket>>>,
    ping_task: Option<PingTaskHandler>,
}

impl InnerPinger {

    fn send_now(&mut self) {
        if let Ok(socket) = self.socket.try_borrow() {
            if let Some(socket) = socket.as_ref() {
                self.num += 1;

                let msg =
                    serde_json::to_string(&Heartbeat::Ping(self.num)).unwrap();

                socket.send_with_str(&msg).unwrap();
            }
        }
    }
}

struct PingTaskHandler {
    ping_closure: Closure<dyn FnMut()>,
    interval_handler: IntervalHandle,
}

impl Pinger {
    fn new() -> Self {
        Self(Rc::new(RefCell::new(InnerPinger {
            num: 0,
            pong_at: None,
            socket: Rc::new(RefCell::new(None)),
            ping_task: None,
        })))
    }

    fn set_pong_at(&self, at: f64) {
        self.0.borrow_mut().pong_at = Some(at);
    }

    fn start(&self, socket: Rc<RefCell<Option<WebSocket>>>) {
        let mut inner = self.0.borrow_mut();
        inner.socket = socket;
        inner.send_now();

        let inner_rc = Rc::clone(&self.0);
        let do_ping = Closure::wrap(Box::new(move || {
            inner_rc.borrow_mut().send_now();
        }) as Box<dyn FnMut()>);

        let interval_id = window()
            .set_interval_with_callback_and_timeout_and_arguments_0(
                do_ping.as_ref().unchecked_ref(),
                3000,
            )
            .unwrap();

        inner.ping_task = Some(PingTaskHandler {
            ping_closure: do_ping,
            interval_handler: IntervalHandle(interval_id),
        });
    }

    fn stop(&self) {
        self.0.borrow_mut().ping_task.take();
    }
}

struct Command {}

#[derive(Deserialize, Serialize)]
pub enum Heartbeat {
    /// `ping` message that WebSocket client is expected to send to the server
    /// periodically.
    #[serde(rename = "ping")]
    Ping(usize),
    /// `pong` message that server answers with to WebSocket client in response
    /// to received `ping` message.
    #[serde(rename = "pong")]
    Pong(usize),
}

#[wasm_bindgen]
pub struct Jason {
    transport: Option<Transport>,
}

#[wasm_bindgen]
pub struct SessionHandler {
    tx: UnboundedSender<Command>,
    rx: UnboundedReceiver<Command>,
}

impl SessionHandler {
    fn new() -> SessionHandler {
        let (tx, rx) = unbounded();

        SessionHandler { tx, rx }
    }
}

#[wasm_bindgen]
impl Jason {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        set_panic_hook();

        Self { transport: None }
    }

    pub fn init_session(&mut self, token: String) -> SessionHandler {
        let mut transport = Transport::new(token);
        transport.init();

        let handler = SessionHandler::new();

        transport.add_sub(handler.tx.clone());

        self.transport = Some(transport);

        handler
    }
}

use futures::sync::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use js_sys::JSON;
use serde::{Deserialize, Serialize};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{console, MessageEvent, WebSocket, Window};

// When the `console_error_panic_hook` feature is enabled, we can call the
// `set_panic_hook` function at least once during initialization, and then
// we will get better error messages if our code ever panics.
//
// For more details see
// https://github.com/rustwasm/console_error_panic_hook#readme
#[cfg(feature = "console_error_panic_hook")]
pub use console_error_panic_hook::set_once as set_panic_hook;
use core::borrow::Borrow;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

fn window() -> Window {
    // cannot use lazy_static since window is !Sync
    // safe to unwrap
    web_sys::window().unwrap()
}

struct Transport {
    sock: Option<Rc<WebSocket>>,
    token: String,
    pinger: Rc<RefCell<Pinger>>,
    subs: Vec<UnboundedSender<Command>>,
}

struct Pinger {
    num: usize,
    socket: Option<Rc<WebSocket>>,
    hearbeat_handler: Option<HearbeatHandler>
}

struct HearbeatHandler {
    closure: Closure<Box<dyn FnMut>>,
    interva_id: u32,
}

impl Transport {
    fn new(token: String) -> Self {
        Transport {
            sock: None,
            token,
            subs: vec![],
            pinger: Rc::new(RefCell::new(Pinger::new())),
        }
    }

    fn init(&mut self) {
        let socket = WebSocket::new(&self.token).unwrap();
        let socket = Rc::new(socket);

        let socket_rc = Rc::clone(&socket);
        let pinger_rc = Rc::clone(&self.pinger);
        let on_open = move || {
            console::log(&js_sys::Array::from(&JsValue::from_str(
                "socket opened",
            )));
            pinger_rc.try_borrow_mut().unwrap().start(socket_rc);
        };
        let on_open: Closure<FnMut() -> ()> = Closure::once(on_open);

        let on_message = Closure::wrap(Box::new(move |event: MessageEvent| {
            let payload = event.data();
            console::log(&js_sys::Array::from(&payload));
        }) as Box<dyn FnMut(_)>);

        socket
            .add_event_listener_with_callback(
                "open",
                on_open.as_ref().unchecked_ref(),
            )
            .unwrap();
        on_open.forget();

        socket
            .add_event_listener_with_callback(
                "message",
                on_message.as_ref().unchecked_ref(),
            )
            .unwrap();
        on_message.forget();

        self.sock = Some(socket)
    }

    fn add_sub(&mut self, sub: UnboundedSender<Command>) {
        self.subs.push(sub);
    }
}

impl Pinger {
    fn new() -> Self {
        Pinger {
            num: 0,
            socket: None,
            hearbeat_handler: None
        }
    }

    fn start(&mut self, socket: Rc<WebSocket>) {
        self.socket = Some(socket);
        self.send_now();

        console::log(&js_sys::Array::from(&JsValue::from_str(
            "pinger started",
        )));

        let closure = Closure::wrap(Box::new(|| {
            console::log(&js_sys::Array::from(&JsValue::from_str("ping")));
            //            self.send_now();
        }) as Box<dyn FnMut()>);

        let interva_id = window()
            .set_interval_with_callback_and_timeout_and_arguments_0(
                ping.as_ref().unchecked_ref(),
                3000,
            )
            .unwrap();
        self.hearbeat_handler = Some(HearbeatHandler {
            closure,
            interva_id
        });
    }

    fn send_now(&mut self) {
        match self.socket.as_ref() {
            None => {}
            Some(socket) => {
                let a: &Rc<WebSocket> = socket;
                self.num += 1;

                let msg =
                    serde_json::to_string(&Heartbeat::Ping(self.num)).unwrap();

                socket.send_with_str(&msg).unwrap();
            }
        };
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

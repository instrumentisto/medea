use futures::sync::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use wasm_bindgen::prelude::*;

mod transport;
mod utils;

use transport::{protocol::*, Transport};

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

#[wasm_bindgen]
pub struct Jason {
    transport: Option<Transport>,
}

#[wasm_bindgen]
pub struct SessionHandler {
    tx: UnboundedSender<Command>,
    _rx: UnboundedReceiver<Command>,
}

impl SessionHandler {
    fn new() -> SessionHandler {
        let (tx, rx) = unbounded();

        SessionHandler { tx, _rx: rx }
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
        let mut transport = Transport::new(token, 3000);
        transport.init();

        let handler = SessionHandler::new();

        transport.add_sub(handler.tx.clone());

        self.transport = Some(transport);

        handler
    }
}

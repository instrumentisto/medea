use wasm_bindgen::prelude::*;

use std::rc::Rc;

use crate::{
    api::{session::Session, Handle},
    set_panic_hook,
    transport::Transport,
};

#[wasm_bindgen]
#[derive(Default)]
pub struct Jason {
    transport: Option<Rc<Transport>>,
    sessions: Vec<Rc<Session>>,
}

#[wasm_bindgen]
impl Jason {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        set_panic_hook();
        Self::default()
    }

    pub fn init_session(&mut self, token: String) -> Result<Handle, JsValue> {
        let mut transport = Transport::new(token, 3000);
        transport.init()?;
        let transport = Rc::new(transport);

        let session = Session::new(Rc::clone(&transport));
        session.subscribe(&transport);

        let handle = session.new_handle();

        self.sessions.push(Rc::new(session));
        self.transport = Some(transport);

        Ok(handle)
    }
}

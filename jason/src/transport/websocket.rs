use wasm_bindgen::{prelude::*, JsCast};
use web_sys::{CloseEvent, MessageEvent, WebSocket as BackingSocket};

use std::{cell::RefCell, convert::TryFrom};

use crate::{
    transport::{
        protocol::{InMsg, OutMsg},
        CloseMsg,
    },
    utils::WasmErr,
};

struct InnerSocket {
    socket: BackingSocket,
    on_open: Option<Closure<dyn FnMut()>>,
    on_message: Option<Closure<dyn FnMut(MessageEvent)>>,
    on_close: Option<Closure<dyn FnMut(CloseEvent)>>,
}

pub struct WebSocket(RefCell<InnerSocket>);

impl InnerSocket {
    fn new(url: &str) -> Result<Self, WasmErr> {
        Ok(Self {
            socket: BackingSocket::new(url)?,
            on_open: None,
            on_message: None,
            on_close: None,
        })
    }
}

impl WebSocket {
    pub fn new(url: &str) -> Result<Self, WasmErr> {
        Ok(Self(RefCell::new(InnerSocket::new(url)?)))
    }

    pub fn on_open<F>(&self, f: F) -> Result<(), WasmErr>
    where
        F: (FnOnce()) + 'static,
    {
        let mut inner_ref = self.0.borrow_mut();

        let closure: Closure<FnMut()> = Closure::once(f);
        inner_ref.socket.add_event_listener_with_callback(
            "open",
            closure.as_ref().unchecked_ref(),
        )?;

        inner_ref.on_open = Some(closure);
        Ok(())
    }

    pub fn on_message<F>(&self, mut f: F) -> Result<(), WasmErr>
    where
        F: (FnMut(Result<InMsg, WasmErr>)) + 'static,
    {
        let mut inner_ref = self.0.borrow_mut();

        let closure = Closure::wrap(Box::new(move |msg: MessageEvent| {
            let parsed = InMsg::try_from(&msg);

            f(parsed);
        }) as Box<dyn FnMut(MessageEvent)>);

        inner_ref.socket.add_event_listener_with_callback(
            "message",
            closure.as_ref().unchecked_ref(),
        )?;
        inner_ref.on_message = Some(closure);
        Ok(())
    }

    pub fn on_close<F>(&self, f: F) -> Result<(), WasmErr>
    where
        F: (FnOnce(CloseMsg)) + 'static,
    {
        let mut inner_ref = self.0.borrow_mut();

        let closure: Closure<FnMut(CloseEvent)> =
            Closure::once(move |msg: CloseEvent| {
                let parsed = CloseMsg::from(&msg);

                f(parsed);
            });

        inner_ref.socket.add_event_listener_with_callback(
            "close",
            closure.as_ref().unchecked_ref(),
        )?;
        inner_ref.on_close = Some(closure);
        Ok(())
    }

    pub fn send(&self, msg: &OutMsg) -> Result<(), WasmErr> {
        self.0
            .borrow()
            .socket
            .send_with_str(&serde_json::to_string(msg)?)
            .map_err(WasmErr::from)
    }

    pub fn _close(&self, reason: &str) {
        if let Err(err) = self
            .0
            .borrow()
            .socket
            .close_with_code_and_reason(1000, reason)
        {
            WasmErr::from(err).log_err();
        }
    }
}

impl Drop for WebSocket {
    fn drop(&mut self) {
        self.0
            .borrow()
            .socket
            .close_with_code_and_reason(1001, "Dropped suddenly")
            .is_ok();
    }
}

impl TryFrom<&MessageEvent> for InMsg {
    type Error = WasmErr;

    fn try_from(msg: &MessageEvent) -> Result<Self, Self::Error> {
        let payload = msg
            .data()
            .as_string()
            .ok_or_else(|| WasmErr::from_str("Payload is not string"))?;

        serde_json::from_str::<Self>(&payload).map_err(WasmErr::from)
    }
}

impl From<&CloseEvent> for CloseMsg {
    fn from(event: &CloseEvent) -> Self {
        let code: u16 = event.code();
        let body = format!("{}:{}", code, event.reason());
        match code {
            1000 => CloseMsg::Normal(body),
            _ => CloseMsg::Disconnect(body),
        }
    }
}

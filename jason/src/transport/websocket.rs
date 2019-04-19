use wasm_bindgen::{prelude::*, JsCast};
use web_sys::{CloseEvent, Event, MessageEvent, WebSocket as BackingSocket};

use std::{rc::Rc, cell::RefCell, convert::TryFrom};

use crate::{
    transport::{
        protocol::{InMsg, OutMsg},
        CloseMsg,
    },
    utils::WasmErr,
};

enum State {
    CONNECTING = 0,
    OPEN = 1,
    CLOSING = 2,
    CLOSED = 3,
    NONE = 4,
}

impl TryFrom<u16> for State {
    type Error = WasmErr;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(State::CONNECTING),
            1 => Ok(State::OPEN),
            2 => Ok(State::CLOSING),
            3 => Ok(State::CLOSED),
            4 => Ok(State::NONE),
            _ => Err(WasmErr::Other(
                format!("Could not cast {} to State variant", value).into(),
            )),
        }
    }
}

struct InnerSocket {
    socket: BackingSocket,
    socket_state: State,
    on_open: Option<Closure<dyn FnMut(Event)>>,
    on_message: Option<Closure<dyn FnMut(MessageEvent)>>,
    on_close: Option<Closure<dyn FnMut(CloseEvent)>>,
    on_error: Option<Closure<dyn FnMut(Event)>>,
}

pub struct WebSocket(Rc<RefCell<InnerSocket>>);

impl InnerSocket {
    fn new(url: &str) -> Result<Rc<RefCell<Self>>, WasmErr> {
        Ok(Rc::new(RefCell::new(Self {
            socket: BackingSocket::new(url)?,
            socket_state: State::NONE,
            on_open: None,
            on_message: None,
            on_close: None,
            on_error: None,
        })))
    }

    fn update_state(&mut self) {
        match State::try_from(self.socket.ready_state()) {
            Ok(new_state) => self.socket_state = new_state,
            Err(err) => {
                // unreachable, unless some vendor will break enum
                err.log_err()
            }
        };
    }
}

impl WebSocket {
    pub fn new(url: &str) -> Result<Self, WasmErr> {
        let inner = InnerSocket::new(url)?;

        let inner_rc = Rc::clone(&inner);
        let mut inner_ref = inner.borrow_mut();

        let on_error: Closure<FnMut(Event)> =
            Closure::once(move |_: Event| {
                inner_rc.borrow_mut().update_state();
            });

        inner_ref.socket.add_event_listener_with_callback(
            "close",
            on_error.as_ref().unchecked_ref(),
        )?;

        inner_ref.on_error = Some(on_error);

        drop(inner_ref);
        Ok(Self(inner))
    }

    pub fn on_open<F>(&self, f: F) -> Result<(), WasmErr>
    where
        F: (FnOnce()) + 'static,
    {
        let inner_rc = Rc::clone(&self.0);
        let mut inner_ref = self.0.borrow_mut();

        let closure: Closure<FnMut(Event)> = Closure::once(move |_| {
            inner_rc.borrow_mut().update_state();
            f();
        });

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
        let inner_rc = Rc::clone(&self.0);
        let mut inner_ref = self.0.borrow_mut();

        let closure: Closure<FnMut(CloseEvent)> =
            Closure::once(move |msg: CloseEvent| {
                inner_rc.borrow_mut().update_state();
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
        let inner = self.0.borrow();

        match inner.socket_state {
            State::OPEN => {
                inner.socket
                    .send_with_str(&serde_json::to_string(msg)?)
                    .map_err(WasmErr::from)
            },
            _ => { Err(WasmErr::from_str("Underlying socket is closed")) },
        }
    }

    pub fn _close(self, reason: &str) {
        let inner = self.0.borrow();

        match inner.socket_state {
            State::CONNECTING | State::OPEN => {
                if let Err(err) =
                    inner.socket.close_with_code_and_reason(1000, reason)
                {
                    WasmErr::from(err).log_err();
                }
            }
            _ => {}
        }
    }
}

impl Drop for WebSocket {
    fn drop(&mut self) {
        WasmErr::from_str("Drop for WebSocket").log_err();

        if let Err(e) = self
            .0
            .borrow()
            .socket
            .close_with_code_and_reason(1000, "Dropped suddenly")
        {
            WasmErr::from(e).log_err();
        }

        WasmErr::from_str("Drop for WebSocket").log_err();
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

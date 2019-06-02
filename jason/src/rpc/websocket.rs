//! ['WebSocket'](https://developer.mozilla.org/ru/docs/WebSockets)
//! transport wrapper.
use futures::future::{Future, IntoFuture};
use protocol::{ClientMsg, ServerMsg};
use web_sys::{CloseEvent, Event, MessageEvent, WebSocket as BackingSocket};

use std::{cell::RefCell, convert::TryFrom, rc::Rc, thread};

use crate::{
    rpc::CloseMsg,
    utils::{EventListener, WasmErr},
};

/// State of websocket.
#[derive(Debug)]
enum State {
    CONNECTING,
    OPEN,
    CLOSING,
    CLOSED,
}

impl State {
    /// Returns true if socket can be closed.
    pub fn can_close(&self) -> bool {
        match self {
            State::CONNECTING | State::OPEN => true,
            _ => false,
        }
    }
}

impl TryFrom<u16> for State {
    type Error = WasmErr;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(State::CONNECTING),
            1 => Ok(State::OPEN),
            2 => Ok(State::CLOSING),
            3 => Ok(State::CLOSED),
            _ => Err(WasmErr::Other(
                format!("Could not cast {} to State variant", value).into(),
            )),
        }
    }
}

struct InnerSocket {
    socket: Rc<BackingSocket>,
    socket_state: State,
    on_open: Option<EventListener<BackingSocket, Event>>,
    on_message: Option<EventListener<BackingSocket, MessageEvent>>,
    on_close: Option<EventListener<BackingSocket, CloseEvent>>,
    on_error: Option<EventListener<BackingSocket, Event>>,
}

pub struct WebSocket(Rc<RefCell<InnerSocket>>);

impl InnerSocket {
    fn new(url: &str) -> Result<Self, WasmErr> {
        Ok(Self {
            socket_state: State::CONNECTING,
            socket: Rc::new(BackingSocket::new(url)?),
            on_open: None,
            on_message: None,
            on_close: None,
            on_error: None,
        })
    }

    /// Checks underlying WebSocket state and updates socket_state.
    fn update_state(&mut self) {

        println!("{:?}", thread::current());

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
    /// Initiates new WebSocket connection. Resolves only when underlying
    /// connection becomes active.
    pub fn new(url: &str) -> impl Future<Item = Self, Error = WasmErr> {
        let (tx_close, rx_close) = futures::oneshot();
        let (tx_open, rx_open) = futures::oneshot();

        InnerSocket::new(url)
            .into_future()
            .and_then(move |socket| {
                let socket = Rc::new(RefCell::new(socket));
                let mut socket_mut = socket.borrow_mut();

                let inner = Rc::clone(&socket);
                socket_mut.on_close = Some(EventListener::new_once(
                    Rc::clone(&socket_mut.socket),
                    "close",
                    move |_| {
                        inner.borrow_mut().update_state();
                        let _ = tx_close.send(());
                    },
                )?);

                let inner = Rc::clone(&socket);
                socket_mut.on_open = Some(EventListener::new_once(
                    Rc::clone(&socket_mut.socket),
                    "open",
                    move |_| {
                        inner.borrow_mut().update_state();
                        let _ = tx_open.send(());
                    },
                )?);

                drop(socket_mut);
                Ok(Self(socket))
            })
            .and_then(move |socket| {
                rx_open
                    .then(move |_| {
                        let mut socket_mut = socket.0.borrow_mut();
                        socket_mut.on_open.take();
                        socket_mut.on_close.take();
                        drop(socket_mut);
                        Ok(socket)
                    })
                    .select(rx_close.then(|_| {
                        Err(WasmErr::from_str("Failed to init WebSocket"))
                    }))
                    .map(|(socket, _)| socket)
                    .map_err(|(err, _)| err)
            })
    }

    /// Set handler on receive message from server.
    pub fn on_message<F>(&self, mut f: F) -> Result<(), WasmErr>
    where
        F: (FnMut(Result<ServerMsg, WasmErr>)) + 'static,
    {
        let mut inner_mut = self.0.borrow_mut();
        inner_mut.on_message = Some(EventListener::new_mut(
            Rc::clone(&inner_mut.socket),
            "message",
            move |msg| {
                let parsed =
                    ServerMessage::try_from(&msg).map(std::convert::Into::into);

                f(parsed);
            },
        )?);
        Ok(())
    }

    /// Set handler on close socket.
    pub fn on_close<F>(&self, f: F) -> Result<(), WasmErr>
    where
        F: (FnOnce(CloseMsg)) + 'static,
    {
        let mut inner_mut = self.0.borrow_mut();
        let inner = Rc::clone(&self.0);
        inner_mut.on_close = Some(EventListener::new_once(
            Rc::clone(&inner_mut.socket),
            "close",
            move |msg: CloseEvent| {
                inner.borrow_mut().update_state();
                let parsed = CloseMsg::from(&msg);

                f(parsed);
            },
        )?);
        Ok(())
    }

    /// Send message to server.
    pub fn send(&self, msg: &ClientMsg) -> Result<(), WasmErr> {
        let inner = self.0.borrow();

        match inner.socket_state {
            State::OPEN => inner
                .socket
                .send_with_str(&serde_json::to_string(msg)?)
                .map_err(WasmErr::from),
            _ => Err(WasmErr::from_str("Underlying socket is closed")),
        }
    }
}

impl Drop for WebSocket {
    fn drop(&mut self) {
        let mut inner = self.0.borrow_mut();
        if inner.socket_state.can_close() {
            inner.on_open.take();
            inner.on_error.take();
            inner.on_message.take();
            inner.on_close.take();

            if let Err(err) = inner
                .socket
                .close_with_code_and_reason(1000, "Dropped unexpectedly")
            {
                WasmErr::from(err).log_err();
            }
        }
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

macro_attr! {
    #[derive(NewtypeFrom!)]
    pub struct ServerMessage(ServerMsg);
}

impl TryFrom<&MessageEvent> for ServerMessage {
    type Error = WasmErr;

    fn try_from(msg: &MessageEvent) -> Result<Self, Self::Error> {
        let payload = msg
            .data()
            .as_string()
            .ok_or_else(|| WasmErr::from_str("Payload is not string"))?;

        serde_json::from_str::<ServerMsg>(&payload)
            .map_err(WasmErr::from)
            .map(Self::from)
    }
}

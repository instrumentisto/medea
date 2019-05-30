/// Represents connection with specific [`Member`].
use futures::future::Either;
use futures::{
    future::{Future, IntoFuture},
    stream::Stream,
};
use protocol::Command;
use protocol::{Event, IceCandidate, Track};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use crate::{
    media::{MediaManager, MediaStreamHandle, PeerId, PeerRepository, Sdp},
    rpc::RPCClient,
    utils::{Callback, WasmErr},
};

#[allow(clippy::module_name_repetitions)]
#[wasm_bindgen]
pub struct ConnectionHandle(Weak<RefCell<InnerConnection>>);

#[wasm_bindgen]
impl ConnectionHandle {
    pub fn on_remote_stream(&mut self, f: js_sys::Function) {
        if let Some(inner) = self.0.upgrade() {
            inner.borrow_mut().on_remote_stream.set_func(f);
        } else {
            let f: Callback<i32, WasmErr> = f.into();
            f.call_err(WasmErr::from_str("Detached state"));
        }
    }
}

pub struct Connection(Rc<RefCell<InnerConnection>>);

impl Connection {
    pub fn new(member_id: u64) -> Self {
        Self(Rc::new(RefCell::new(InnerConnection {
            remote_member: member_id,
            on_remote_stream: Rc::new(Callback::new()),
        })))
    }

    pub fn new_handle(&self) -> ConnectionHandle {
        ConnectionHandle(Rc::downgrade(&self.0))
    }
}

struct InnerConnection {
    remote_member: u64,
    on_remote_stream: Rc<Callback<MediaStreamHandle, WasmErr>>,
}

impl InnerConnection {}

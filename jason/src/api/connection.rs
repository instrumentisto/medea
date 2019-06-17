/// Represents connection with specific remote [`Member`].
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use wasm_bindgen::prelude::*;

use crate::{
    media::{MediaStream, MediaStreamHandle},
    utils::{Callback, WasmErr},
};

/// Represents connection with specific remote [`Member`].
///
/// Shared between JS-side handle ([`ConnectionHandle`])
/// and Rust-side handle ([`Connection`]).
struct InnerConnection {
    remote_member: u64,
    on_remote_stream: Callback<MediaStreamHandle>,
}

/// [`InnerConnection`] handle accessible from js.
#[allow(clippy::module_name_repetitions)]
#[wasm_bindgen]
pub struct ConnectionHandle(Weak<RefCell<InnerConnection>>);

#[wasm_bindgen]
impl ConnectionHandle {
    /// Sets callback, that will be invoked on remote [`Member`] media stream
    /// arrival.
    pub fn on_remote_stream(
        &mut self,
        f: js_sys::Function,
    ) -> Result<(), JsValue> {
        if let Some(inner) = self.0.upgrade() {
            inner.borrow_mut().on_remote_stream.set_func(f);
            Ok(())
        } else {
            Err(WasmErr::build_from_str("Detached state").into())
        }
    }

    /// Returns remote member Id.
    pub fn member_id(&self) -> Result<u64, JsValue> {
        if let Some(inner) = self.0.upgrade() {
            Ok(inner.borrow().remote_member)
        } else {
            Err(WasmErr::build_from_str("Detached state").into())
        }
    }
}

/// [`InnerConnection`] handle being used by Rust external modules.
pub struct Connection(Rc<RefCell<InnerConnection>>);

impl Connection {
    pub fn new(member_id: u64) -> Self {
        Self(Rc::new(RefCell::new(InnerConnection {
            remote_member: member_id,
            on_remote_stream: Callback::default(),
        })))
    }

    /// Creates new [`ConnectionHandle`] used by JS side.
    pub fn new_handle(&self) -> ConnectionHandle {
        ConnectionHandle(Rc::downgrade(&self.0))
    }

    /// Pass new [`MediaStream`] received from related remote [`Member`].
    pub fn new_remote_stream(&self, stream: &MediaStream) {
        self.0.borrow().on_remote_stream.call(stream.new_handle());
    }
}

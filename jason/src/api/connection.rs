//! Connection with specific remote `Member`.

use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use wasm_bindgen::prelude::*;

use crate::{
    peer::RemoteMediaStream,
    utils::{Callback, HandlerDetachedError},
};

/// Actual data of a connection with a specific remote [`Member`].
///
/// Shared between JS side ([`ConnectionHandle`]) and
/// Rust side ([`Connection`]).
struct InnerConnection {
    on_remote_stream: Callback<RemoteMediaStream>,
}

/// Connection with a specific remote `Member`, that is used on JS side.
///
/// Actually, represents a [`Weak`]-based handle to `InnerConnection`.
#[wasm_bindgen]
pub struct ConnectionHandle(Weak<RefCell<InnerConnection>>);

#[wasm_bindgen]
impl ConnectionHandle {
    /// Sets callback, which will be invoked on remote `Member` media stream
    /// arrival.
    pub fn on_remote_stream(
        &mut self,
        f: js_sys::Function,
    ) -> Result<(), JsValue> {
        upgrade_or_detached!(self.0)
            .map(|inner| inner.borrow_mut().on_remote_stream.set_func(f))
    }
}

/// Connection with a specific remote [`Member`], that is used on Rust side.
///
/// Actually, represents a handle to [`InnerConnection`].
pub(crate) struct Connection(Rc<RefCell<InnerConnection>>);

impl Connection {
    /// Instantiates new [`Connection`] for a given [`Member`].
    #[inline]
    pub(crate) fn new() -> Self {
        Self(Rc::new(RefCell::new(InnerConnection {
            on_remote_stream: Callback::default(),
        })))
    }

    /// Creates new [`ConnectionHandle`] for using [`Connection`] on JS side.
    #[inline]
    pub(crate) fn new_handle(&self) -> ConnectionHandle {
        ConnectionHandle(Rc::downgrade(&self.0))
    }

    /// Invoke `on_remote_stream` [`Connection`]'s callback
    /// for a given [`MediaStream`] received from a related remote [`Member`].
    #[inline]
    pub(crate) fn on_remote_stream(&self, stream: RemoteMediaStream) {
        self.0.borrow().on_remote_stream.call(stream);
    }
}

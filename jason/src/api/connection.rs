//! Connection with specific remote [`Member`].

use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use medea_client_api_proto::PeerId;
use wasm_bindgen::prelude::*;

use crate::{
    media::{MediaStream, MediaStreamHandle},
    utils::{Callback, WasmErr},
};

/// Actual data of a connection with a specific remote [`Member`].
///
/// Shared between JS side ([`ConnectionHandle`]) and
/// Rust side ([`Connection`]).
struct InnerConnection {
    remote_member: PeerId,
    on_remote_stream: Callback<MediaStreamHandle>,
}

/// Connection with a specific remote [`Member`], that is used on JS side.
///
/// Actually, represents a [`Weak`]-based handle to [`InnerConnection`].
#[allow(clippy::module_name_repetitions)]
#[wasm_bindgen]
pub struct ConnectionHandle(Weak<RefCell<InnerConnection>>);

#[wasm_bindgen]
impl ConnectionHandle {
    /// Sets callback, which will be invoked on remote [`Member`] media stream
    /// arrival.
    pub fn on_remote_stream(
        &mut self,
        f: js_sys::Function,
    ) -> Result<(), JsValue> {
        self.0
            .upgrade()
            .map(|conn| {
                conn.borrow_mut().on_remote_stream.set_func(f);
            })
            .ok_or_else(|| WasmErr::from("Detached state").into())
    }

    /// Returns ID of the remote [`Member`].
    pub fn member_id(&self) -> Result<u64, JsValue> {
        self.0
            .upgrade()
            .map(|conn| conn.borrow().remote_member.0)
            .ok_or_else(|| WasmErr::from("Detached state").into())
    }
}

/// Connection with a specific remote [`Member`], that is used on Rust side.
///
/// Actually, represents a handle to [`InnerConnection`].
pub(crate) struct Connection(Rc<RefCell<InnerConnection>>);

impl Connection {
    /// Instantiates new [`Connection`] for a given [`Member`].
    #[inline]
    pub(crate) fn new(member_id: PeerId) -> Self {
        Self(Rc::new(RefCell::new(InnerConnection {
            remote_member: member_id,
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
    pub(crate) fn on_remote_stream(&self, stream: &MediaStream) {
        self.0.borrow().on_remote_stream.call(stream.new_handle());
    }
}

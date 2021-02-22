use derive_more::From;
use wasm_bindgen::prelude::*;

use crate::core;

/// Connection with a specific remote `Member`, that is used on JS side.
///
/// Actually, represents a [`Weak`]-based handle to `InnerConnection`.
#[wasm_bindgen]
#[derive(From)]
pub struct ConnectionHandle(core::ConnectionHandle);

#[wasm_bindgen]
impl ConnectionHandle {
    /// Sets callback, which will be invoked when this `Connection` will close.
    pub fn on_close(&self, cb: js_sys::Function) -> Result<(), JsValue> {
        self.0.on_close(cb).map_err(JsValue::from)
    }

    /// Returns remote `Member` ID.
    pub fn get_remote_member_id(&self) -> Result<String, JsValue> {
        self.0.get_remote_member_id().map_err(JsValue::from)
    }

    /// Sets callback, which will be invoked when new [`remote::Track`] will be
    /// added to this [`Connection`].
    pub fn on_remote_track_added(
        &self,
        cb: js_sys::Function,
    ) -> Result<(), JsValue> {
        self.0.on_remote_track_added(cb).map_err(JsValue::from)
    }

    /// Sets callback, which will be invoked when connection quality score will
    /// be updated by server.
    pub fn on_quality_score_update(
        &self,
        cb: js_sys::Function,
    ) -> Result<(), JsValue> {
        self.0.on_quality_score_update(cb).map_err(JsValue::from)
    }
}

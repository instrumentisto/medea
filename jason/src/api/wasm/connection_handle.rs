use derive_more::From;
use wasm_bindgen::prelude::*;

use crate::{api::JasonError, core};

/// Connection with a specific remote `Member`, that is used on JS side.
///
/// Like all handlers it contains weak reference to object that is managed by
/// Rust, so its methods will fail if weak reference could not be upgraded.
#[wasm_bindgen]
#[derive(From)]
pub struct ConnectionHandle(core::ConnectionHandle);

#[wasm_bindgen]
impl ConnectionHandle {
    /// Sets callback, which will be invoked when this [`Connection`] will
    /// close.
    ///
    /// [`Connection`]: core::Connection
    pub fn on_close(&self, cb: js_sys::Function) -> Result<(), JsValue> {
        self.0
            .on_close(cb.into())
            .map_err(JasonError::from)
            .map_err(JsValue::from)
    }

    /// Returns remote `Member` ID.
    pub fn get_remote_member_id(&self) -> Result<String, JsValue> {
        self.0
            .get_remote_member_id()
            .map_err(JasonError::from)
            .map_err(JsValue::from)
    }

    /// Sets callback, which will be invoked when new [`RemoteMediaTrack`] will
    /// be added to this [`Connection`].
    ///
    /// [`RemoteMediaTrack`]: crate::api::RemoteMediaTrack
    /// [`Connection`]: core::Connection
    pub fn on_remote_track_added(
        &self,
        cb: js_sys::Function,
    ) -> Result<(), JsValue> {
        self.0
            .on_remote_track_added(cb.into())
            .map_err(JasonError::from)
            .map_err(JsValue::from)
    }

    /// Sets callback, which will be invoked when connection quality score will
    /// be updated by server.
    pub fn on_quality_score_update(
        &self,
        cb: js_sys::Function,
    ) -> Result<(), JsValue> {
        self.0
            .on_quality_score_update(cb.into())
            .map_err(JasonError::from)
            .map_err(JsValue::from)
    }
}

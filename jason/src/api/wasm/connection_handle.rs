//! Connection with a specific remote `Member` used on JS side.

use derive_more::From;
use wasm_bindgen::prelude::*;

use crate::{api, connection};

/// Connection with a specific remote `Member`, that is used on JS side.
///
/// Like all the handles it contains a weak reference to the object that is
/// managed by Rust, so its methods will fail if a weak reference could not be
/// upgraded.
#[wasm_bindgen]
#[derive(From)]
pub struct ConnectionHandle(connection::ConnectionHandle);

#[wasm_bindgen]
impl ConnectionHandle {
    /// Sets callback, invoked when this [`Connection`] is closed.
    ///
    /// [`Connection`]: connection::Connection
    pub fn on_close(&self, cb: js_sys::Function) -> Result<(), JsValue> {
        self.0
            .on_close(cb.into())
            .map_err(api::Error::from)
            .map_err(Into::into)
    }

    /// Returns ID of the remote `Member`.
    pub fn get_remote_member_id(&self) -> Result<String, JsValue> {
        self.0
            .get_remote_member_id()
            .map_err(api::Error::from)
            .map_err(Into::into)
    }

    /// Sets callback, invoked when a new [`RemoteMediaTrack`] is added to this
    /// [`Connection`].
    ///
    /// [`Connection`]: connection::Connection
    /// [`RemoteMediaTrack`]: crate::api::RemoteMediaTrack
    pub fn on_remote_track_added(
        &self,
        cb: js_sys::Function,
    ) -> Result<(), JsValue> {
        self.0
            .on_remote_track_added(cb.into())
            .map_err(api::Error::from)
            .map_err(Into::into)
    }

    /// Sets callback, invoked when connection quality score is updated by a
    /// server.
    pub fn on_quality_score_update(
        &self,
        cb: js_sys::Function,
    ) -> Result<(), JsValue> {
        self.0
            .on_quality_score_update(cb.into())
            .map_err(api::Error::from)
            .map_err(Into::into)
    }
}

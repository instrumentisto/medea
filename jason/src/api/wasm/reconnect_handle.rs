//! JS side handle for reconnections with a media server.

use derive_more::From;
use js_sys::Promise;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

use crate::rpc;

use super::JasonError;

/// Handle that JS side can reconnect to a media server with when a connection
/// is lost.
///
/// This handle is passed into a [`RoomHandle.on_connection_loss`] callback.
///
/// Like all the handles it contains a weak reference to the object that is
/// managed by Rust, so its methods will fail if a weak reference could not be
/// upgraded.
///
/// [`RoomHandle.on_connection_loss`]: crate::api::RoomHandle.on_connection_loss
#[wasm_bindgen]
#[derive(Clone, From)]
pub struct ReconnectHandle(rpc::ReconnectHandle);

#[wasm_bindgen]
impl ReconnectHandle {
    /// Tries to reconnect after the provided delay in milliseconds.
    ///
    /// If [`RpcSession`] is already reconnecting then a new reconnection
    /// attempt won't be performed. Instead, it will wait for the first
    /// reconnection attempt result and use it.
    ///
    /// [`RpcSession`]: rpc::RpcSession
    pub fn reconnect_with_delay(&self, delay_ms: u32) -> Promise {
        let this = self.0.clone();
        future_to_promise(async move {
            this.reconnect_with_delay(delay_ms)
                .await
                .map_err(JasonError::from)?;
            Ok(JsValue::UNDEFINED)
        })
    }

    /// Tries to reconnect a [`RpcSession`] in a loop with a growing backoff
    /// delay.
    ///
    /// The first attempt to reconnect is guaranteed to happen no earlier than
    /// `starting_delay_ms`.
    ///
    /// Also, it guarantees that delay between reconnection attempts won't be
    /// greater than `max_delay_ms`.
    ///
    /// After each reconnection attempt, delay between reconnections will be
    /// multiplied by the given `multiplier` until it reaches `max_delay_ms`.
    ///
    /// If [`RpcSession`] is already reconnecting then new reconnection attempt
    /// won't be performed. Instead, it will wait for the first reconnection
    /// attempt result and use it here.
    ///
    /// If `multiplier` is negative number than `multiplier` will be considered
    /// as `0.0`.
    ///
    /// [`RpcSession`]: rpc::RpcSession
    pub fn reconnect_with_backoff(
        &self,
        starting_delay_ms: u32,
        multiplier: f32,
        max_delay: u32,
        stop_on_max: bool,
    ) -> Promise {
        let this = self.0.clone();
        future_to_promise(async move {
            this.reconnect_with_backoff(
                starting_delay_ms,
                multiplier.into(),
                max_delay,
                stop_on_max,
            )
            .await
            .map_err(JasonError::from)?;
            Ok(JsValue::UNDEFINED)
        })
    }
}

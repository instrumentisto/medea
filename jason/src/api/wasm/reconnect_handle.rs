use derive_more::From;
use js_sys::Promise;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

use crate::{api::JasonError, core};

/// Handle that JS side can reconnect to the Medea media server on
/// a connection loss with.
///
/// This handle will be provided into `Room.on_connection_loss` callback.
#[wasm_bindgen]
#[derive(Clone, From)]
pub struct ReconnectHandle(core::ReconnectHandle);

#[wasm_bindgen]
impl ReconnectHandle {
    /// Tries to reconnect after the provided delay in milliseconds.
    ///
    /// If [`RpcSession`] is already reconnecting then new reconnection attempt
    /// won't be performed. Instead, it will wait for the first reconnection
    /// attempt result and use it here.
    ///
    /// [`RpcSession`]: core::rpc::RpcSession
    pub fn reconnect_with_delay(&self, delay_ms: u32) -> Promise {
        let this = self.0.clone();
        future_to_promise(async move {
            this.reconnect_with_delay(delay_ms)
                .await
                .map_err(JasonError::from)?;
            Ok(JsValue::UNDEFINED)
        })
    }

    /// Tries to reconnect [`RpcSession`] in a loop with a growing backoff
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
    /// [`RpcSession`]: core::rpc::RpcSession
    pub fn reconnect_with_backoff(
        &self,
        starting_delay_ms: u32,
        multiplier: f32,
        max_delay: u32,
    ) -> Promise {
        let this = self.0.clone();
        future_to_promise(async move {
            this.reconnect_with_backoff(
                starting_delay_ms,
                multiplier,
                max_delay,
            )
            .await
            .map_err(JasonError::from)?;
            Ok(JsValue::UNDEFINED)
        })
    }
}

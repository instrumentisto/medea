//! Reconnection for [`RpcSession`].

use std::{rc::Weak, time::Duration};

use derive_more::Display;
use js_sys::Promise;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

use crate::{
    rpc::{BackoffDelayer, RpcSession},
    utils::{delay_for, HandlerDetachedError, JasonError, JsCaused, JsError},
};

/// Error which indicates that [`RpcSession`]'s (which this [`ReconnectHandle`]
/// tries to reconnect) token is `None`.
#[derive(Debug, Display, JsCaused)]
struct NoTokenError;

/// Handle that JS side can reconnect to the Medea media server on
/// a connection loss with.
///
/// This handle will be provided into `Room.on_connection_loss` callback.
#[wasm_bindgen]
#[derive(Clone)]
pub struct ReconnectHandle(Weak<dyn RpcSession>);

impl ReconnectHandle {
    /// Instantiates new [`ReconnectHandle`] from the given [`RpcSession`]
    /// reference.
    #[inline]
    #[must_use]
    pub fn new(rpc: Weak<dyn RpcSession>) -> Self {
        Self(rpc)
    }
}

#[wasm_bindgen]
impl ReconnectHandle {
    /// Tries to reconnect after the provided delay in milliseconds.
    ///
    /// If [`RpcSession`] is already reconnecting then new reconnection attempt
    /// won't be performed. Instead, it will wait for the first reconnection
    /// attempt result and use it here.
    pub fn reconnect_with_delay(&self, delay_ms: u32) -> Promise {
        let rpc = Clone::clone(&self.0);
        future_to_promise(async move {
            delay_for(Duration::from_millis(u64::from(delay_ms)).into()).await;

            let rpc = upgrade_or_detached!(rpc, JsValue)?;
            rpc.reconnect()
                .await
                .map_err(|e| JsValue::from(JasonError::from(e)))?;

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
    pub fn reconnect_with_backoff(
        &self,
        starting_delay_ms: u32,
        multiplier: f32,
        max_delay: u32,
    ) -> Promise {
        let rpc = self.0.clone();
        future_to_promise(async move {
            let mut backoff_delayer = BackoffDelayer::new(
                Duration::from_millis(u64::from(starting_delay_ms)).into(),
                multiplier,
                Duration::from_millis(u64::from(max_delay)).into(),
            );
            backoff_delayer.delay().await;
            while upgrade_or_detached!(rpc, JsValue)?
                .reconnect()
                .await
                .is_err()
            {
                backoff_delayer.delay().await;
            }

            Ok(JsValue::UNDEFINED)
        })
    }
}

//! Implementation of reconnector for a [`RpcClient`].

use std::{rc::Weak, time::Duration};

use derive_more::Display;
use js_sys::Promise;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

use crate::{
    rpc::{BackoffDelayer, RpcClient},
    utils::{delay_for, HandlerDetachedError, JasonError, JsCaused, JsError},
};

/// Error which indicates that [`RpcClient`]'s (which this [`ReconnectHandle`]
/// tries to reconnect) token is `None`.
#[derive(Debug, Display, JsCaused)]
struct NoTokenError;

/// Object with which JS side can reconnect to the Medea media server on
/// connection loss.
///
/// This object will be provided into `Room.on_connection_loss` callback.
#[wasm_bindgen]
#[derive(Clone)]
pub struct ReconnectorHandle(Weak<dyn RpcClient>);

impl ReconnectorHandle {
    /// Creates new [`ReconnectorHandle`] with which JS side can reconnect
    /// provided [`RpcClient`] on connection loss.
    pub fn new(rpc: Weak<dyn RpcClient>) -> Self {
        Self(rpc)
    }
}

#[wasm_bindgen]
impl ReconnectorHandle {
    /// Tries to reconnect after provided delay in milliseconds.
    ///
    /// If [`RpcClient`] is already reconnecting then new reconenction try
    /// wouldn't be performed. Instead of this, this function will wait for
    /// first reconnection try result and use it here.
    pub fn reconnect_with_delay(&self, delay_ms: u32) -> Promise {
        let rpc = Clone::clone(&self.0);
        future_to_promise(async move {
            delay_for(Duration::from_millis(u64::from(delay_ms)).into()).await;

            let rpc = upgrade_or_detached!(rpc, JsValue)?;
            let token = rpc
                .get_token()
                .ok_or_else(|| new_js_error!(NoTokenError => JsValue))?;
            rpc.connect(token)
                .await
                .map_err(|e| JsValue::from(JasonError::from(e)))?;

            Ok(JsValue::UNDEFINED)
        })
    }

    /// Tries to reconnect [`RpcClient`] in a loop with growing
    /// delay until it will not be reconnected.
    ///
    /// The first attempt to reconnect is guaranteed to happen no earlier than
    /// `starting_delay_ms`.
    ///
    /// Also this function guarantees that delay between reconnection attempts
    /// will be not greater than `max_delay_ms`.
    ///
    /// After each reconnection try, delay between reconnections will be
    /// multiplied by `multiplier` until it reaches `max_delay_ms`.
    ///
    /// If [`RpcClient`] is already reconnecting then new reconenction try
    /// wouldn't be performed. Instead of this, this function will wait for
    /// first reconnection try result and use it here.
    pub fn reconnect_with_backoff(
        &self,
        starting_delay_ms: u32,
        multiplier: f32,
        max_delay: u32,
    ) -> Promise {
        let rpc = self.0.clone();
        future_to_promise(async move {
            let token = upgrade_or_detached!(rpc, JsValue)?
                .get_token()
                .ok_or_else(|| new_js_error!(NoTokenError => JsValue))?;

            let mut backoff_delayer = BackoffDelayer::new(
                Duration::from_millis(u64::from(starting_delay_ms)).into(),
                multiplier,
                Duration::from_millis(u64::from(max_delay)).into(),
            );
            backoff_delayer.delay().await;
            while let Err(_) = upgrade_or_detached!(rpc, JsValue)?
                .connect(token.clone())
                .await
            {
                backoff_delayer.delay().await;
            }

            Ok(JsValue::UNDEFINED)
        })
    }
}

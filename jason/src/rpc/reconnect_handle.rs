//! Implementation of reconnector for the [`ReconnectableRpcClient`].

use std::{rc::Weak, time::Duration};

use derive_more::Display;
use js_sys::Promise;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

use crate::{
    rpc::{BackoffDelayer, RpcClient, RpcClientError},
    utils::{resolve_after, JasonError, JasonWeakHandler, JsCaused, JsError},
};
use js_sys::Math::max;
use std::rc::Rc;
use tracerr::Traced;

// TODO: why not reuse DetachedState error?

/// [`RpcClient`] which will be reconnected is gone.
#[derive(Debug, Display, JsCaused)]
struct RpcClientGoneError;

// TODO: how is this object responsible for ReconnectableRpcClient reconnecting?
//       what is the difference between this object and a raw RpcClient weak
//       reference?

/// Object which responsible for [`ReconnectableRpcClient`] reconnecting.
///
/// Mainly used on JS side through [`ReconnectorHandle`].
pub struct Reconnector(Weak<dyn RpcClient>);

impl Reconnector {
    /// Returns new [`Reconnector`] for provided [`ReconnectableRpcClient`].
    pub fn new(rpc: Weak<dyn RpcClient>) -> Self {
        Self(rpc)
    }

    /// Returns new [`ReconnectorHandle`] which points to this [`Reconnector`].
    pub fn new_handle(&self) -> ReconnectorHandle {
        ReconnectorHandle(Clone::clone(&self.0))
    }
}

async fn reconnect(
    rpc: Rc<dyn RpcClient>,
) -> Result<(), Traced<RpcClientError>> {
    // TODO: PANIC
    let token = rpc.get_token().unwrap();
    rpc.connect(token).await.map_err(|e| tracerr::new!(e))
}

/// JS side handle for [`Reconnector`].
#[wasm_bindgen]
#[derive(Clone)]
pub struct ReconnectorHandle(Weak<dyn RpcClient>);

#[wasm_bindgen]
impl ReconnectorHandle {
    /// Tries to reconnect after provided delay in milliseconds.
    pub fn reconnect_with_delay(&self, delay_ms: u32) -> Promise {
        let rpc = Clone::clone(&self.0);
        future_to_promise(async move {
            resolve_after(Duration::from_millis(u64::from(delay_ms)).into())
                .await;

            let rpc = Weak::upgrade(&rpc).ok_or_else(|| {
                JsValue::from(JasonError::from(tracerr::new!(
                    RpcClientGoneError
                )))
            })?;
            let token = rpc.get_token().unwrap();
            rpc.connect(token)
                .await
                .map_err(|e| JsValue::from(JasonError::from(e)))?;

            Ok(JsValue::NULL)
        })
    }

    /// Tries to reconnect [`ReconnectableRpcClient`] in a loop with growing
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

            while let Err(e) =
                reconnect(rpc.upgrade().ok_or_else(|| JsValue::NULL)?).await
            {
                backoff_delayer.delay().await;
            }

            Ok(JsValue::NULL)
        })
    }
}

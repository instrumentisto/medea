use std::rc::{Rc, Weak};

use js_sys::Promise;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

use crate::{
    rpc::ReconnectableRpcClient,
    utils::{resolve_after, JasonError, JasonWeakHandler as _, JsDuration},
};
use std::time::Duration;

struct Inner {
    rpc: Weak<dyn ReconnectableRpcClient>,
}

pub struct Reconnector(Rc<Inner>);

impl Reconnector {
    pub fn new(rpc: Weak<dyn ReconnectableRpcClient>) -> Self {
        Self(Rc::new(Inner { rpc }))
    }

    pub fn new_handle(&self) -> ReconnectorHandle {
        ReconnectorHandle(Rc::downgrade(&self.0))
    }
}

#[wasm_bindgen]
#[derive(Clone)]
pub struct ReconnectorHandle(Weak<Inner>);

impl ReconnectorHandle {
    pub(self) fn new(inner: Weak<Inner>) -> Self {
        Self(inner)
    }
}

#[wasm_bindgen]
impl ReconnectorHandle {
    /// Tries to reconnect after provided delay.
    ///
    /// Delay is in milliseconds.
    pub fn reconnect(&self, delay_ms: u64) -> Promise {
        let this = self.clone();
        future_to_promise(async move {
            resolve_after(Duration::from_millis(delay_ms).into()).await?;
            let inner = this.0.upgrade_handler::<JsValue>()?;
            let rpc: Rc<dyn ReconnectableRpcClient> = Weak::upgrade(&inner.rpc)
                .ok_or_else(|| JsValue::from_str("RpcClient is gone"))?;
            rpc.reconnect()
                .await
                .map_err(|e| JsValue::from(JasonError::from(e)))?;

            Ok(JsValue::NULL)
        })
    }

    /// Tries to reconnect [`RpcTransport`] in a loop with delay until
    /// it will not be reconnected or deadline not be reached.
    pub fn reconnect_with_backoff(
        &self,
        starting_delay: u64,
        multiplier: f32,
        max_delay_ms: u64,
    ) -> Promise {
        let this = self.clone();
        future_to_promise(async move {
            let inner = this.0.upgrade_handler::<JsValue>()?;
            let rpc = Weak::upgrade(&inner.rpc)
                .ok_or_else(|| JsValue::from_str("RpcClient is gone."))?;
            rpc.reconnect_with_backoff(
                Duration::from_millis(starting_delay).into(),
                multiplier,
                Duration::from_millis(max_delay_ms).into(),
            )
            .await
            .map_err(|e| JsValue::from(JasonError::from(e)))?;

            Ok(JsValue::NULL)
        })
    }
}

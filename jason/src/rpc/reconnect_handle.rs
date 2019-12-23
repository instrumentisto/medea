use std::{
    rc::{Rc, Weak},
    time::Duration,
};

use derive_more::Display;
use js_sys::Promise;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

use crate::{
    rpc::ReconnectableRpcClient,
    utils::{resolve_after, JasonError, JasonWeakHandler, JsCaused, JsError},
};

struct Inner {
    /// Client which may be reconnected with this [`Reconnector`].
    rpc: Weak<dyn ReconnectableRpcClient>,
}

pub struct Reconnector(Rc<Inner>);

impl Reconnector {
    /// Returns new [`Reconnector`] for provided [`ReconnectableRpcClient`].
    pub fn new(rpc: Weak<dyn ReconnectableRpcClient>) -> Self {
        Self(Rc::new(Inner { rpc }))
    }

    /// Returns new [`ReconnectorHandle`] which points to this [`Reconnector`].
    pub fn new_handle(&self) -> ReconnectorHandle {
        ReconnectorHandle(Rc::downgrade(&self.0))
    }
}

/// JS side handle for [`ReconnectorLock`].
#[wasm_bindgen]
#[derive(Clone)]
pub struct ReconnectorHandle(Weak<Inner>);

#[derive(Debug, Display, JsCaused)]
enum ReconnectorError {
    /// [`RpcClient`] which will be reconnected is gone.
    RpcClientGone,
}

#[wasm_bindgen]
impl ReconnectorHandle {
    /// Tries to reconnect after provided delay.
    ///
    /// Delay is in milliseconds.
    pub fn reconnect(&self, delay_ms: u32) -> Promise {
        let this = self.clone();
        future_to_promise(async move {
            let inner = this.0.upgrade_handler::<JsValue>()?;
            resolve_after(Duration::from_millis(u64::from(delay_ms)).into())
                .await?;

            Weak::upgrade(&inner.rpc)
                .ok_or_else(|| {
                    JsValue::from(JasonError::from(tracerr::new!(
                        ReconnectorError::RpcClientGone
                    )))
                })?
                .reconnect()
                .await
                .map_err(|e| JsValue::from(JasonError::from(e)))?;

            Ok(JsValue::NULL)
        })
    }

    /// Tries to reconnect [`RpcTransport`] in a loop with delay until
    /// it will not be reconnected or deadline not be reached.
    pub fn reconnect_with_backoff(
        &self,
        starting_delay: u32,
        multiplier: f32,
        max_delay_ms: u32,
    ) -> Promise {
        let this = self.clone();
        future_to_promise(async move {
            let inner = this.0.upgrade_handler::<JsValue>()?;

            let rpc = Weak::upgrade(&inner.rpc).ok_or_else(|| {
                JsValue::from(JasonError::from(tracerr::new!(
                    ReconnectorError::RpcClientGone
                )))
            })?;

            rpc.reconnect_with_backoff(
                Duration::from_millis(u64::from(starting_delay)).into(),
                multiplier,
                Duration::from_millis(u64::from(max_delay_ms)).into(),
            )
            .await
            .map_err(|e| JsValue::from(JasonError::from(e)))?;

            Ok(JsValue::NULL)
        })
    }
}

use std::rc::{Rc, Weak};

use derive_more::{Deref, Display};
use js_sys::Promise;
use tracerr::Traced;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

use crate::{
    rpc::ReconnectableRpcClient,
    utils::{
        resolve_after, JasonError, JasonWeakHandler as _, JsCaused, JsDuration,
        JsError,
    },
};
use std::{cell::Cell, time::Duration};

struct Inner {
    rpc: Weak<dyn ReconnectableRpcClient>,
    is_busy: Cell<bool>,
}

pub struct Reconnector(Rc<Inner>);

impl Reconnector {
    pub fn new(rpc: Weak<dyn ReconnectableRpcClient>) -> Self {
        Self(Rc::new(Inner {
            rpc,
            is_busy: Cell::new(false),
        }))
    }

    pub fn new_handle(&self) -> ReconnectorHandle {
        ReconnectorHandle(ReconnectorLock::new(Rc::downgrade(&self.0)))
    }
}

#[wasm_bindgen]
#[derive(Clone)]
pub struct ReconnectorHandle(ReconnectorLock);

#[derive(Debug, Display, JsCaused)]
enum ReconnectorError {
    Busy,
    RpcClientGone,
    Detached,
}

#[derive(Deref)]
struct ReconnectorGuard(Rc<Inner>);

impl ReconnectorGuard {
    pub fn new(inner: Rc<Inner>) -> Self {
        Self(inner)
    }
}

impl Drop for ReconnectorGuard {
    fn drop(&mut self) {
        self.0.is_busy.set(false);
    }
}

#[derive(Clone)]
struct ReconnectorLock(Weak<Inner>);

impl ReconnectorLock {
    pub fn new(inner: Weak<Inner>) -> Self {
        Self(inner)
    }

    pub fn lock(&self) -> Result<ReconnectorGuard, Traced<ReconnectorError>> {
        let inner = self
            .0
            .upgrade()
            .ok_or_else(|| tracerr::new!(ReconnectorError::Detached))?;
        if inner.is_busy.get() {
            return Err(tracerr::new!(ReconnectorError::Busy));
        }
        inner.is_busy.set(true);

        Ok(ReconnectorGuard::new(inner))
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
            let inner = this
                .0
                .lock()
                .map_err(|e| JsValue::from(JasonError::from(e)))?;

            resolve_after(Duration::from_millis(delay_ms).into()).await?;

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
        starting_delay: u64,
        multiplier: f32,
        max_delay_ms: u64,
    ) -> Promise {
        let this = self.clone();
        future_to_promise(async move {
            let inner = this
                .0
                .lock()
                .map_err(|e| JsValue::from(JasonError::from(e)))?;

            Weak::upgrade(&inner.rpc)
                .ok_or_else(|| {
                    JsValue::from(JasonError::from(tracerr::new!(
                        ReconnectorError::RpcClientGone
                    )))
                })?
                .reconnect_with_backoff(
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

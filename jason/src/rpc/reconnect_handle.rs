use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use js_sys::Promise;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

use crate::{
    rpc::{ReconnectableRpcClient, RpcClient, WeakWebsocketRpcClient},
    utils::{resolve_after, JasonError, JasonWeakHandler},
};

struct Inner {
    rpc: Weak<dyn ReconnectableRpcClient>,
}

pub struct Reconnector(Rc<Inner>);

impl Reconnector {
    pub fn new(rpc: Weak<dyn ReconnectableRpcClient>) -> Self {
        Self(Rc::new(Inner { rpc }))
    }

    pub fn new_handle(&self) -> ReconnectionHandle {
        ReconnectionHandle(Rc::downgrade(&self.0))
    }
}

#[wasm_bindgen]
#[derive(Clone)]
pub struct ReconnectionHandle(Weak<Inner>);

impl ReconnectionHandle {
    pub(self) fn new(inner: Weak<Inner>) -> Self {
        Self(inner)
    }
}

#[wasm_bindgen]
impl ReconnectionHandle {
    pub fn reconnect(&self, delay_ms: i32) -> Promise {
        let this = self.clone();
        future_to_promise(async move {
            resolve_after(delay_ms).await?;
            let inner = this.0.upgrade_handler::<JsValue>()?;
            let rpc: Rc<dyn ReconnectableRpcClient> = Weak::upgrade(&inner.rpc)
                .ok_or_else(|| JsValue::from_str("RpcClient is gone"))?;
            rpc.reconnect()
                .await
                .map_err(|e| JsValue::from(JasonError::from(e)))?;

            Ok(JsValue::NULL)
        })
    }
}

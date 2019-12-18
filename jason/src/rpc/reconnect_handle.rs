use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use js_sys::Promise;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

use crate::{
    rpc::{RpcClient, WeakWebsocketRpcClient},
    utils::resolve_after,
};
use crate::utils::JasonWeakHandler;

struct Inner {
    rpc: Weak<RefCell<dyn RpcClient>>,
}

#[wasm_bindgen]
#[derive(Clone)]
pub struct ReconnectionHandle(Weak<RefCell<Inner>>);

#[wasm_bindgen]
impl ReconnectionHandle {
    pub fn reconnect(&self, delay_ms: i32) -> Promise {
        let this = self.clone();
        future_to_promise(async move {
            resolve_after(delay_ms).await?;
            let inner = this.0.upgrade_handler::<JsValue>()?;
            let rpc: Rc<RefCell<dyn RpcClient>> =
                Weak::upgrade(&inner.borrow_mut().rpc)
                    .ok_or_else(|| JsValue::from_str("RpcClient is gone"))?;

            Ok(JsValue::NULL)
        })
    }
}

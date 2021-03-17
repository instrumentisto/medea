//! Tests for the [`medea_jason::utils::resettable_delay_for`].

use std::time::Duration;

use futures::FutureExt;
use medea_jason::utils::resettable_delay_for;
use wasm_bindgen_test::*;

use crate::timeout;

#[wasm_bindgen_test]
async fn delay_is_resolved() {
    let (fut, _) = resettable_delay_for(Duration::from_millis(50), false);
    let fut = fut.shared();

    timeout(40, fut.clone()).await.unwrap_err();
    timeout(40, fut).await.unwrap();
}

#[wasm_bindgen_test]
async fn delay_is_not_resolved_if_handle_is_stopped() {
    let (fut, handle) = resettable_delay_for(Duration::from_millis(50), false);
    handle.stop();
    timeout(100, fut).await.unwrap_err();
}

#[wasm_bindgen_test]
async fn delay_is_resolved_on_stop_and_reset() {
    let (fut, handle) = resettable_delay_for(Duration::from_millis(50), false);
    let fut = fut.shared();
    handle.stop();
    timeout(60, fut.clone()).await.unwrap_err();
    handle.reset();
    // to make sure that timer is reset and not paused.
    timeout(40, fut.clone()).await.unwrap_err();
    timeout(60, fut).await.unwrap();
}

#[wasm_bindgen_test]
async fn resetting_without_stop() {
    let (fut, handle) = resettable_delay_for(Duration::from_millis(50), false);
    let fut = fut.shared();

    timeout(40, fut.clone()).await.unwrap_err();
    handle.reset();
    timeout(40, fut.clone()).await.unwrap_err();
    handle.reset();

    timeout(60, fut).await.unwrap();
}

#[wasm_bindgen_test]
async fn stop_and_drop_handle() {
    let (fut, handle) = resettable_delay_for(Duration::from_millis(50), false);
    handle.stop();
    drop(handle);
    timeout(100, fut).await.unwrap_err();
}

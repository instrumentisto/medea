//! Tests for the [`medea_jason::utils::freezeable_delay_for`].

use std::time::Duration;

use medea_jason::utils::{delay_for, freezeable_delay_for};
use wasm_bindgen_futures::spawn_local;
use wasm_bindgen_test::*;

use crate::await_with_timeout;

/// Tests that [`FreezeableDelayHandle`] really freezes timer [`Future`].
#[wasm_bindgen_test]
async fn freezes() {
    let (fut, handle) = freezeable_delay_for(Duration::from_millis(100));
    handle.freeze();
    await_with_timeout(Box::pin(fut), 110).await.unwrap_err();
}

/// Tests that [`FreezeableDelayHandle`] will start countdown from the beginning
/// after freeze and unfreeze.
#[wasm_bindgen_test]
async fn unfreezes() {
    let (fut, handle) = freezeable_delay_for(Duration::from_millis(100));
    spawn_local(async move {
        delay_for(Duration::from_millis(50).into()).await;
        handle.freeze();
        delay_for(Duration::from_millis(100).into()).await;
    });
    await_with_timeout(Box::pin(fut), 160).await.unwrap();
}

//! Tests for [`medea_jason::rpc::BackoffDelayer`].

use std::time::Duration;

use medea_jason::rpc::BackoffDelayer;
use wasm_bindgen_test::*;

use crate::timeout;

wasm_bindgen_test_configure!(run_in_browser);

/// Tests that `delay` multiplies by provided `multiplier`.
#[wasm_bindgen_test]
async fn multiplier_works() {
    let mut delayer = BackoffDelayer::new(
        Duration::from_millis(10).into(),
        1.5,
        Duration::from_millis(100).into(),
    );
    timeout(13, delayer.delay()).await.unwrap();
    timeout(18, delayer.delay()).await.unwrap();
    timeout(25, delayer.delay()).await.unwrap();
}

/// Tests that `delay` wouldn't be greater than provided `max_delay`.
#[wasm_bindgen_test]
async fn max_delay_works() {
    let mut delayer = BackoffDelayer::new(
        Duration::from_millis(50).into(),
        2.0,
        Duration::from_millis(100).into(),
    );
    timeout(53, delayer.delay()).await.unwrap();
    timeout(103, delayer.delay()).await.unwrap();
    timeout(103, delayer.delay()).await.unwrap();
}

/// Tests that multiplication by negative `multiplier` will be calculated as
/// `0`.
#[wasm_bindgen_test]
async fn negative_multiplier() {
    let mut delayer = BackoffDelayer::new(
        Duration::from_millis(10).into(),
        -2.0,
        Duration::from_millis(100).into(),
    );
    timeout(13, delayer.delay()).await.unwrap();
    timeout(3, delayer.delay()).await.unwrap();
    timeout(3, delayer.delay()).await.unwrap();
}

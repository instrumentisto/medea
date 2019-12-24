use std::time::Duration;

use futures::FutureExt as _;
use medea_jason::{
    rpc::{BackoffDelayer, BackoffDelayerError},
    utils::JsDuration,
};
use wasm_bindgen_test::*;

use crate::await_with_timeout;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn multiplier_works() {
    let mut delayer = BackoffDelayer::new(
        Duration::from_millis(10).into(),
        1.5,
        Duration::from_millis(100).into(),
    );
    await_with_timeout(Box::pin(delayer.delay()), 13)
        .await
        .unwrap()
        .unwrap();
    await_with_timeout(Box::pin(delayer.delay()), 18)
        .await
        .unwrap()
        .unwrap();
    await_with_timeout(Box::pin(delayer.delay()), 25)
        .await
        .unwrap()
        .unwrap();
}

#[wasm_bindgen_test]
async fn max_delay_works() {
    let mut delayer = BackoffDelayer::new(
        Duration::from_millis(50).into(),
        2.0,
        Duration::from_millis(100).into(),
    );
    await_with_timeout(Box::pin(delayer.delay()), 53)
        .await
        .unwrap()
        .unwrap();
    await_with_timeout(Box::pin(delayer.delay()), 103)
        .await
        .unwrap()
        .unwrap();
    await_with_timeout(Box::pin(delayer.delay()), 103)
        .await
        .unwrap()
        .unwrap();
}

#[wasm_bindgen_test]
async fn negative_multiplier() {
    let mut delayer = BackoffDelayer::new(
        Duration::from_millis(10).into(),
        -2.0,
        Duration::from_millis(100).into(),
    );
    await_with_timeout(Box::pin(delayer.delay()), 13)
        .await
        .unwrap()
        .unwrap();
    await_with_timeout(Box::pin(delayer.delay()), 3)
        .await
        .unwrap()
        .unwrap();
    await_with_timeout(Box::pin(delayer.delay()), 3)
        .await
        .unwrap()
        .unwrap();
}

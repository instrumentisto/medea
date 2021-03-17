//! Tests for the [`TransitableStateController`].

use futures::StreamExt;
use medea_jason::peer::{media_exchange_state, MediaExchangeStateController};
use wasm_bindgen_test::wasm_bindgen_test;

use crate::timeout;

/// Tests that [`TransitableStateController`] will freeze transition timeout for
/// the new transitions.
#[wasm_bindgen_test]
async fn controller_inheritance_delay_freeze() {
    let controller = MediaExchangeStateController::new(
        media_exchange_state::Stable::Enabled,
    );
    controller.stop_transition_timeout();
    controller.transition_to(media_exchange_state::Stable::Disabled);

    timeout(600, controller.subscribe_stable().next())
        .await
        .unwrap_err();
}

/// Tests that [`TransitableStateController`] will unfreeze frozen on start
/// transition timeout.
#[wasm_bindgen_test]
async fn unfreezes_inheritance_delay_freeze() {
    let controller = MediaExchangeStateController::new(
        media_exchange_state::Stable::Enabled,
    );
    controller.stop_transition_timeout();
    controller.transition_to(media_exchange_state::Stable::Disabled);
    controller.reset_transition_timeout();

    let rollbacked_state = timeout(600, controller.subscribe_stable().next())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(rollbacked_state, media_exchange_state::Stable::Enabled);
}

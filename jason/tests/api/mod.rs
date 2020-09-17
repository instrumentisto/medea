mod connection;
mod room;

use std::rc::Rc;

use futures::{
    channel::{mpsc, oneshot},
    stream, StreamExt,
};
use medea_client_api_proto::{CloseReason, RpcSettings, ServerMsg};
use medea_jason::{
    rpc::{
        websocket::{MockRpcTransport, TransportState},
        CloseMsg, RpcTransport, WebSocketRpcClient,
    },
    Jason,
};
use medea_reactive::ObservableCell;
use wasm_bindgen::closure::Closure;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;

use crate::timeout;

wasm_bindgen_test_configure!(run_in_browser);

/// [`ServerMsg::RpcSettings`] which will be sent in the all tests from this
/// module.
const RPC_SETTINGS: ServerMsg = ServerMsg::RpcSettings(RpcSettings {
    idle_timeout_ms: 5_000,
    ping_interval_ms: 2_000,
});

/// Checks that only one [`Rc`] to the [`RpcClient`] exists.
#[wasm_bindgen_test]
async fn only_one_strong_rpc_rc_exists() {
    let jason = Jason::default();
    let ws =
        Rc::new(WebSocketRpcClient::new(Box::new(move |_| {
            Box::pin(async move {
                let mut transport = MockRpcTransport::new();
                transport.expect_on_message().times(3).returning_st(
                    move || Box::pin(stream::once(async { RPC_SETTINGS })),
                );
                transport.expect_send().return_once(|_| Ok(()));
                transport
                    .expect_set_close_reason()
                    .times(1)
                    .return_once(|_| ());
                transport.expect_on_state_change().return_once_st(move || {
                    Box::pin(stream::once(async { TransportState::Open }))
                });
                let transport = Rc::new(transport);
                Ok(transport as Rc<dyn RpcTransport>)
            })
        })));

    let room = jason.inner_init_room(ws.clone());
    room.on_failed_local_stream(Closure::once_into_js(|| {}).into())
        .unwrap();
    room.on_connection_loss(Closure::once_into_js(|| {}).into())
        .unwrap();
    JsFuture::from(room.join("ws://example.com".to_string()))
        .await
        .unwrap();

    assert_eq!(Rc::strong_count(&ws), 2);
}

/// Checks that [`RpcClient`] was dropped on [`JasonHandle::dispose`] call.
#[wasm_bindgen_test]
async fn rpc_dropped_on_jason_dispose() {
    let jason = Jason::default();
    let (test_tx, mut test_rx) = mpsc::unbounded();
    let ws =
        Rc::new(WebSocketRpcClient::new(Box::new(move |_| {
            let test_tx = test_tx.clone();
            Box::pin(async move {
                let mut transport = MockRpcTransport::new();
                transport.expect_on_message().times(3).returning_st(
                    move || Box::pin(stream::once(async { RPC_SETTINGS })),
                );
                transport.expect_send().return_once(|_| Ok(()));
                transport.expect_set_close_reason().times(1).return_once(
                    move |reason| {
                        test_tx.unbounded_send(reason).unwrap();
                    },
                );
                transport.expect_on_state_change().return_once_st(move || {
                    Box::pin(stream::once(async { TransportState::Open }))
                });
                let transport = Rc::new(transport);
                Ok(transport as Rc<dyn RpcTransport>)
            })
        })));

    let room = jason.inner_init_room(ws);
    room.on_failed_local_stream(Closure::once_into_js(|| {}).into())
        .unwrap();
    room.on_connection_loss(Closure::once_into_js(|| {}).into())
        .unwrap();
    JsFuture::from(room.join("ws://example.com".to_string()))
        .await
        .unwrap();

    jason.dispose();
    timeout(300, test_rx.next()).await.unwrap();
}

/// Tests that [`Room`] will trigger [`RoomHandle::on_close`] callback on
/// [`RpcTransport`] close.
#[wasm_bindgen_test]
async fn room_closes_on_rpc_transport_close() {
    let jason = Jason::default();
    let on_state_change_mock =
        Rc::new(ObservableCell::new(TransportState::Open));
    let ws = Rc::new(WebSocketRpcClient::new(Box::new({
        let on_state_change_mock = on_state_change_mock.clone();
        move |_| {
            let on_state_change_mock = on_state_change_mock.clone();
            Box::pin(async move {
                let mut transport = MockRpcTransport::new();
                transport.expect_on_message().times(3).returning_st(
                    move || Box::pin(stream::once(async { RPC_SETTINGS })),
                );
                transport.expect_send().return_once(|_| Ok(()));
                transport.expect_set_close_reason().return_once(|_| ());
                transport
                    .expect_on_state_change()
                    .return_once_st(move || on_state_change_mock.subscribe());
                let transport = Rc::new(transport);
                Ok(transport as Rc<dyn RpcTransport>)
            })
        }
    })));

    let mut room = jason.inner_init_room(ws);
    room.on_failed_local_stream(Closure::once_into_js(|| {}).into())
        .unwrap();
    room.on_connection_loss(Closure::once_into_js(|| {}).into())
        .unwrap();
    JsFuture::from(room.join("ws://example.com".to_string()))
        .await
        .unwrap();

    let (test_tx, test_rx) = oneshot::channel();
    let closure = wasm_bindgen::closure::Closure::once_into_js(move || {
        test_tx.send(()).unwrap();
    });
    room.on_close(closure.into()).unwrap();

    on_state_change_mock.set(TransportState::Closed(CloseMsg::Normal(
        1200,
        CloseReason::Finished,
    )));

    timeout(300, test_rx).await.unwrap().unwrap();
}

mod connection;
mod room;

use std::rc::Rc;

use futures::{
    channel::{mpsc, oneshot},
    stream, StreamExt,
};
use medea_client_api_proto::{
    ClientMsg, CloseReason, Command, RpcSettings, ServerMsg,
};
use medea_jason::{
    rpc::{
        websocket::{MockRpcTransport, TransportState},
        CloseMsg, RpcTransport, WebSocketRpcClient,
    },
    Jason,
};
use medea_reactive::ObservableCell;
use wasm_bindgen::closure::Closure;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use wasm_bindgen_test::*;

use crate::{delay_for, timeout, yield_now, TEST_ROOM_URL};
use std::cell::RefCell;

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
    let ws = Rc::new(WebSocketRpcClient::new(Box::new(move |_| {
        Box::pin(async move {
            let mut transport = MockRpcTransport::new();
            transport.expect_on_message().times(3).returning_st({
                move || {
                    Box::pin(stream::iter(vec![
                        RPC_SETTINGS,
                        ServerMsg::JoinedRoom {
                            room_id: "room_id".into(),
                            member_id: "member_id".into(),
                        },
                    ]))
                }
            });
            transport.expect_send().returning(|_| Ok(()));
            transport.expect_set_close_reason().return_once(|_| ());
            transport.expect_on_state_change().return_once_st(move || {
                Box::pin(stream::once(async { TransportState::Open }))
            });
            let transport = Rc::new(transport);
            Ok(transport as Rc<dyn RpcTransport>)
        })
    })));
    let jason = Jason::with_rpc_client(ws.clone());

    let room = jason.init_room();
    room.on_failed_local_media(Closure::once_into_js(|| {}).into())
        .unwrap();
    room.on_connection_loss(Closure::once_into_js(|| {}).into())
        .unwrap();
    room.inner_join(TEST_ROOM_URL.to_string()).await.unwrap();

    assert_eq!(Rc::strong_count(&ws), 3);
    jason.dispose();
    assert_eq!(Rc::strong_count(&ws), 1);
}

/// Checks that [`RpcClient`] was dropped on [`JasonHandle::dispose`] call.
#[wasm_bindgen_test]
async fn rpc_dropped_on_jason_dispose() {
    let (test_tx, mut test_rx) = mpsc::unbounded();
    let ws = Rc::new(WebSocketRpcClient::new(Box::new(move |_| {
        let test_tx = test_tx.clone();
        Box::pin(async move {
            let mut transport = MockRpcTransport::new();
            transport.expect_on_message().times(3).returning_st({
                move || {
                    Box::pin(stream::iter(vec![
                        RPC_SETTINGS,
                        ServerMsg::JoinedRoom {
                            room_id: "room_id".into(),
                            member_id: "member_id".into(),
                        },
                    ]))
                }
            });
            transport.expect_send().times(2).returning(|_| Ok(()));
            transport.expect_set_close_reason().times(1).returning(
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
    let jason = Jason::with_rpc_client(ws);

    let room = jason.init_room();
    room.on_failed_local_media(Closure::once_into_js(|| {}).into())
        .unwrap();
    room.on_connection_loss(Closure::once_into_js(|| {}).into())
        .unwrap();
    JsFuture::from(room.join(TEST_ROOM_URL.to_string()))
        .await
        .unwrap();
    jason.dispose();
    drop(room);

    timeout(100, test_rx.next()).await.unwrap();
}

/// Checks that [`Jason::dispose_room`] works correctly.
#[wasm_bindgen_test]
async fn room_dispose_works() {
    let (test_tx, mut test_rx) = mpsc::unbounded();
    let (cmd_tx, mut cmd_rx) = mpsc::unbounded();
    let client_msg_txs = Rc::new(RefCell::new(Vec::new()));
    let ws = Rc::new(WebSocketRpcClient::new({
        let client_msg_txs = client_msg_txs.clone();
        Box::new(move |_| {
            let test_tx = test_tx.clone();
            let cmd_tx = cmd_tx.clone();
            let client_msg_txs = client_msg_txs.clone();
            Box::pin(async move {
                let mut transport = MockRpcTransport::new();
                transport.expect_on_message().returning_st({
                    move || {
                        let (tx, rx) = mpsc::unbounded();
                        tx.unbounded_send(RPC_SETTINGS).unwrap();
                        client_msg_txs.borrow_mut().push(tx);
                        Box::pin(rx)
                    }
                });
                transport.expect_send().returning(move |cmd| {
                    cmd_tx.unbounded_send(cmd.clone()).ok();
                    Ok(())
                });
                transport
                    .expect_set_close_reason()
                    .returning(move |reason| {
                        test_tx.unbounded_send(reason).unwrap();
                    });
                transport.expect_on_state_change().returning(|| {
                    Box::pin(stream::once(async { TransportState::Open }))
                });
                let transport = Rc::new(transport);
                Ok(transport as Rc<dyn RpcTransport>)
            })
        })
    }));
    let jason = Jason::with_rpc_client(ws);

    let room = jason.init_room();
    room.on_failed_local_media(Closure::once_into_js(|| {}).into())
        .unwrap();
    room.on_connection_loss(Closure::once_into_js(|| {}).into())
        .unwrap();
    spawn_local({
        let client_msg_txs = client_msg_txs.clone();
        async move {
            yield_now().await;
            client_msg_txs.borrow().iter().for_each(|tx| {
                tx.unbounded_send(ServerMsg::JoinedRoom {
                    room_id: "room_id".into(),
                    member_id: "member_id".into(),
                })
                .ok();
            });
        }
    });
    JsFuture::from(room.join(TEST_ROOM_URL.to_string()))
        .await
        .unwrap();

    let another_room = jason.init_room();
    another_room
        .on_failed_local_media(Closure::once_into_js(|| {}).into())
        .unwrap();
    another_room
        .on_connection_loss(Closure::once_into_js(|| {}).into())
        .unwrap();
    spawn_local({
        let client_msg_txs = client_msg_txs.clone();
        async move {
            yield_now().await;
            client_msg_txs.borrow().iter().for_each(|tx| {
                tx.unbounded_send(ServerMsg::JoinedRoom {
                    room_id: "another_room_id".into(),
                    member_id: "member_id".into(),
                })
                .ok();
            });
        }
    });
    JsFuture::from(
        another_room.join(
            "ws://example.com/another_room_id/member_id/token".to_string(),
        ),
    )
    .await
    .unwrap();

    assert!(matches!(
        cmd_rx.next().await.unwrap(),
        ClientMsg::JoinRoom { room_id: _, member_id: _, token: _ }
    ));
    assert!(matches!(
        cmd_rx.next().await.unwrap(),
        ClientMsg::JoinRoom { room_id: _, member_id: _, token: _ }
    ));

    jason.dispose_room("room_id");
    assert!(matches!(
        cmd_rx.next().await.unwrap(),
        ClientMsg::LeaveRoom { room_id: _, member_id: _ }
    ));

    jason.dispose_room("another_room_id");
    assert!(matches!(
        cmd_rx.next().await.unwrap(),
        ClientMsg::LeaveRoom { room_id: _, member_id: _ }
    ));

    jason.dispose();

    timeout(100, test_rx.next()).await.unwrap();
}

/// Tests that [`Room`] will trigger [`RoomHandle::on_close`] callback on
/// [`RpcTransport`] close.
#[wasm_bindgen_test]
async fn room_closes_on_rpc_transport_close() {
    let on_state_change_mock =
        Rc::new(ObservableCell::new(TransportState::Open));
    let ws = Rc::new(WebSocketRpcClient::new(Box::new({
        let on_state_change_mock = on_state_change_mock.clone();
        move |_| {
            let on_state_change_mock = on_state_change_mock.clone();
            Box::pin(async move {
                let mut transport = MockRpcTransport::new();
                transport.expect_on_message().times(3).returning_st({
                    move || {
                        Box::pin(stream::iter(vec![
                            RPC_SETTINGS,
                            ServerMsg::JoinedRoom {
                                room_id: "room_id".into(),
                                member_id: "member_id".into(),
                            },
                        ]))
                    }
                });
                transport.expect_send().return_once(|cmd| Ok(()));
                transport.expect_set_close_reason().return_once(|_| ());
                transport
                    .expect_on_state_change()
                    .return_once_st(move || on_state_change_mock.subscribe());
                let transport = Rc::new(transport);
                Ok(transport as Rc<dyn RpcTransport>)
            })
        }
    })));
    let jason = Jason::with_rpc_client(ws);

    let mut room = jason.init_room();
    room.on_failed_local_media(Closure::once_into_js(|| {}).into())
        .unwrap();
    room.on_connection_loss(Closure::once_into_js(|| {}).into())
        .unwrap();
    JsFuture::from(room.join(TEST_ROOM_URL.to_string()))
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

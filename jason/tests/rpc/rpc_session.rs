#![cfg(target_arch = "wasm32")]

use std::{
    rc::Rc,
    str::FromStr,
    sync::atomic::{AtomicBool, Ordering},
};

use futures::stream;
use medea_client_api_proto::{ClientMsg, CloseReason, ServerMsg};
use medea_jason::rpc::{
    websocket::{MockRpcTransport, TransportState},
    ConnectionInfo, RpcSession, RpcTransport, WebSocketRpcClient,
    WebSocketRpcSession,
};
use wasm_bindgen_test::*;

use crate::{rpc::RPC_SETTINGS, timeout, TEST_ROOM_URL};

wasm_bindgen_test_configure!(run_in_browser);

/// Makes sure that `connect` fails immediately if `JoinRoom` request is
/// answered with `LeftRoom` message.
#[wasm_bindgen_test]
async fn could_not_init_socket_err() {
    let session = WebSocketRpcSession::new(Rc::new(WebSocketRpcClient::new(
        Box::new(move |_| {
            Box::pin(async move {
                let mut transport = MockRpcTransport::new();
                transport.expect_on_message().returning_st(|| {
                    Box::pin(stream::iter(vec![
                        RPC_SETTINGS,
                        ServerMsg::LeftRoom {
                            room_id: "room_id".into(),
                            close_reason: CloseReason::InternalError,
                        },
                    ]))
                });
                transport.expect_send().returning(|_| Ok(()));
                transport.expect_set_close_reason().return_once(|_| ());
                transport.expect_on_state_change().return_once_st(move || {
                    Box::pin(stream::once(async { TransportState::Open }))
                });
                let transport = Rc::new(transport);
                Ok(transport as Rc<dyn RpcTransport>)
            })
        }),
    )));

    let connect_fut = Rc::clone(&session)
        .connect(ConnectionInfo::from_str(TEST_ROOM_URL).unwrap());

    timeout(100, connect_fut).await.unwrap().unwrap_err();
}

/// Makes sure that if multiple concurrent `connect` and `reconnect` calls are
/// made, only one `JoinRoom` message will be sent.
#[wasm_bindgen_test]
async fn concurrent_connect_requests() {
    let join_room_sent = Rc::new(AtomicBool::new(false));

    let join_room_sent_clone = Rc::clone(&join_room_sent);
    let session = WebSocketRpcSession::new(Rc::new(WebSocketRpcClient::new({
        Box::new(move |_| {
            let join_room_sent = Rc::clone(&join_room_sent_clone);
            Box::pin(async move {
                let mut transport = MockRpcTransport::new();
                transport.expect_on_message().returning_st(|| {
                    Box::pin(stream::iter(vec![
                        RPC_SETTINGS,
                        ServerMsg::JoinedRoom {
                            room_id: "room_id".into(),
                            member_id: "member_id".into(),
                        },
                    ]))
                });
                let join_room_sent = Rc::clone(&join_room_sent);
                transport.expect_send().returning_st(move |msg| {
                    if matches!(msg, ClientMsg::JoinRoom { .. }) {
                        let already_sent =
                            join_room_sent.fetch_or(true, Ordering::Relaxed);
                        assert!(
                            !already_sent,
                            "only one JoinRoom should be sent"
                        );
                    }
                    Ok(())
                });
                transport.expect_set_close_reason().return_once(|_| ());
                transport.expect_on_state_change().return_once_st(move || {
                    Box::pin(stream::once(async { TransportState::Open }))
                });
                let transport = Rc::new(transport);
                Ok(transport as Rc<dyn RpcTransport>)
            })
        })
    })));

    let connection_info = ConnectionInfo::from_str(TEST_ROOM_URL).unwrap();

    let connect1 = Rc::clone(&session).connect(connection_info.clone());
    let reconnect1 = Rc::clone(&session).reconnect();
    let connect2 = Rc::clone(&session).connect(connection_info);
    let reconnect2 = Rc::clone(&session).reconnect();

    futures::future::try_join_all(vec![
        connect1, reconnect1, connect2, reconnect2,
    ])
    .await
    .unwrap();
    assert!(join_room_sent.load(Ordering::Relaxed));
}

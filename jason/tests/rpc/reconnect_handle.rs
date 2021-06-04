#![cfg(target_arch = "wasm32")]

use std::{
    cell::RefCell,
    rc::{Rc, Weak},
    str::FromStr,
    sync::atomic::{AtomicBool, Ordering},
};

use futures::{
    channel::{mpsc, oneshot},
    future, stream, FutureExt as _, StreamExt as _,
};
use medea_client_api_proto::{
    ClientMsg, CloseReason, Command, Event, ServerMsg,
};
use medea_jason::{
    platform::{
        self, MockRpcTransport, RpcTransport, TransportState,
        WebSocketRpcTransport,
    },
    rpc::{
        CloseMsg, ConnectionInfo, ReconnectError, ReconnectHandle, RpcSession,
        SessionError, WebSocketRpcClient, WebSocketRpcSession,
    },
};
use medea_reactive::ObservableCell;
use wasm_bindgen_test::*;

use crate::{delay_for, rpc::RPC_SETTINGS, timeout, TEST_ROOM_URL};

wasm_bindgen_test_configure!(run_in_browser);

/// Makes sure that [`ReconnectHandle.reconnect_with_backoff()`] works as expected.
#[wasm_bindgen_test]
async fn reconnect_with_backoff() {
    let transport_state = Rc::new(ObservableCell::new(TransportState::Open));

    let state_clone = Rc::clone(&transport_state);
    let session = WebSocketRpcSession::new(Rc::new(WebSocketRpcClient::new(
        Box::new(move |_| {
            let state_clone = Rc::clone(&state_clone);
            Box::pin(async move {
                let mut transport = MockRpcTransport::new();
                transport.expect_on_message().returning_st(|| {
                    Box::pin(stream::iter(vec![
                        RPC_SETTINGS,
                        ServerMsg::Event {
                            room_id: "room_id".into(),
                            event: Event::RoomJoined {
                                member_id: "member_id".into(),
                            },
                        },
                    ]))
                });
                transport.expect_send().returning_st(move |msg| Ok(()));
                transport.expect_set_close_reason().return_once(drop);
                transport
                    .expect_on_state_change()
                    .return_once_st(move || state_clone.subscribe());
                let transport = Rc::new(transport);
                Ok(transport as Rc<dyn RpcTransport>)
            })
        }),
    )));

    let connect_fut = Rc::clone(&session)
        .connect(ConnectionInfo::from_str(TEST_ROOM_URL).unwrap());
    timeout(100, connect_fut).await.unwrap().unwrap();

    transport_state.set(TransportState::Closed(CloseMsg::Abnormal(999)));
    timeout(100, session.on_connection_loss().next())
        .await
        .unwrap()
        .unwrap();
    let handle =
        ReconnectHandle::from(Rc::downgrade(&session) as Weak<dyn RpcSession>);

    // Check that single attempt is made if starting_delay > max_delay.
    let start = instant::Instant::now();
    let err = handle
        .reconnect_with_backoff(100, 999.0, 50, true)
        .await
        .expect_err("supposed to err since transport state didn't change")
        .into_inner();
    let elapsed = start.elapsed().as_millis();
    assert!(elapsed >= 100 && elapsed < 150);
    assert!(matches!(err, ReconnectError::Session(_)));

    // Check that reconnect attempts are made for an expected period.
    let start = instant::Instant::now();
    let err = handle
        .reconnect_with_backoff(10, 1.5, 50, true)
        .await
        .expect_err("supposed to err since transport state didn't change")
        .into_inner();
    let elapsed = start.elapsed().as_millis();
    assert!(elapsed >= 81 && elapsed < 100); // 10 + 15 + 22.5 + 33.75 = 81.25
    assert!(matches!(err, ReconnectError::Session(_)));

    // Check that reconnect returns Ok immediately after a successful attempt.
    platform::spawn({
        let transport_state = Rc::clone(&transport_state);
        async move {
            delay_for(40).await;
            transport_state.set(TransportState::Connecting);
            transport_state.set(TransportState::Open);
        }
    });
    let start = instant::Instant::now();
    let err = handle.reconnect_with_backoff(30, 3.0, 9999, true).await;
    let elapsed = start.elapsed().as_millis();
    assert!(elapsed >= 120 && elapsed < 200); // 30 + 90
    assert!(err.is_ok());

    // Check that `stop_on_max = false` works.
    transport_state.set(TransportState::Closed(CloseMsg::Abnormal(999)));
    timeout(100, session.on_connection_loss().next())
        .await
        .unwrap()
        .unwrap();

    platform::spawn({
        let transport_state = Rc::clone(&transport_state);
        async move {
            delay_for(40).await;
            transport_state.set(TransportState::Connecting);
            transport_state.set(TransportState::Open);
        }
    });
    let start = instant::Instant::now();
    let err = handle.reconnect_with_backoff(1, 1.0, 2, false).await;
    let elapsed = start.elapsed().as_millis();
    assert!(elapsed >= 40 && elapsed < 100);
    assert!(err.is_ok());

    // Check that HandlerDetached is fired if `stop_on_max = false` and all
    // attempts fail.
    transport_state.set(TransportState::Closed(CloseMsg::Abnormal(999)));
    timeout(100, session.on_connection_loss().next())
        .await
        .unwrap()
        .unwrap();

    platform::spawn(async move {
        delay_for(20).await;
        drop(session);
    });
    let start = instant::Instant::now();
    let err = handle
        .reconnect_with_backoff(1, 1.0, 2, false)
        .await
        .expect_err("should err since we drop RpcSession")
        .into_inner();
    let elapsed = start.elapsed().as_millis();
    assert!(elapsed >= 20 && elapsed < 100);
    assert!(matches!(err, ReconnectError::Detached));
}

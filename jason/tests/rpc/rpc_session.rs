#![cfg(target_arch = "wasm32")]

use std::{
    cell::RefCell,
    rc::Rc,
    str::FromStr,
    sync::atomic::{AtomicBool, Ordering},
};

use futures::{future, stream, FutureExt as _, StreamExt as _};
use medea_client_api_proto::{
    ClientMsg, CloseReason, Command, Event, ServerMsg,
};
use medea_jason::rpc::{
    websocket::{MockRpcTransport, TransportState},
    CloseMsg, ConnectionInfo, RpcSession, RpcTransport, SessionError,
    WebSocketRpcClient, WebSocketRpcSession, WebSocketRpcTransport,
};
use wasm_bindgen_test::*;

use crate::{delay_for, rpc::RPC_SETTINGS, timeout, TEST_ROOM_URL};

wasm_bindgen_test_configure!(run_in_browser);

/// Makes sure that `connect` fails immediately if `JoinRoom` request is
/// answered with `RoomLeft` message.
#[wasm_bindgen_test]
async fn could_not_auth_err() {
    let session = WebSocketRpcSession::new(Rc::new(WebSocketRpcClient::new(
        Box::new(move |_| {
            Box::pin(async move {
                let mut transport = MockRpcTransport::new();
                transport.expect_on_message().returning_st(|| {
                    Box::pin(stream::iter(vec![
                        RPC_SETTINGS,
                        ServerMsg::Event {
                            room_id: "room_id".into(),
                            event: Event::RoomLeft {
                                close_reason: CloseReason::InternalError,
                            },
                        },
                    ]))
                });
                transport.expect_send().returning(|_| Ok(()));
                transport.expect_set_close_reason().return_once(drop);
                transport.expect_on_state_change().return_once_st(move || {
                    Box::pin(stream::once(async { TransportState::Open }))
                });
                let transport = Rc::new(transport);
                Ok(transport as Rc<dyn RpcTransport>)
            })
        }),
    )));

    let mut on_normal_close = session.on_normal_close().fuse();
    let mut on_reconnected = session.on_reconnected().fuse();
    let mut on_connection_loss = session.on_connection_loss().fuse();

    let connect_fut = Rc::clone(&session)
        .connect(ConnectionInfo::from_str(TEST_ROOM_URL).unwrap());
    let connect_err = timeout(100, connect_fut)
        .await
        .unwrap()
        .unwrap_err()
        .into_inner();
    assert!(matches!(connect_err, SessionError::AuthorizationFailed));

    // other callbacks should not fire
    futures::select! {
        _ = delay_for(100).fuse() => (),
        _ = on_normal_close => panic!("on_normal_close fired"),
        _ = on_connection_loss.next() => panic!("on_connection_loss fired"),
        _ = on_reconnected.next() => panic!("on_reconnected fired")
    };
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
                        ServerMsg::Event {
                            room_id: "room_id".into(),
                            event: Event::RoomJoined {
                                member_id: "member_id".into(),
                            },
                        },
                    ]))
                });
                let join_room_sent = Rc::clone(&join_room_sent);
                transport.expect_send().returning_st(move |msg| {
                    if matches!(
                        msg,
                        ClientMsg::Command {
                            command: Command::JoinRoom { .. },
                            ..
                        }
                    ) {
                        let already_sent =
                            join_room_sent.fetch_or(true, Ordering::Relaxed);
                        assert!(
                            !already_sent,
                            "only one JoinRoom should be sent"
                        );
                    }
                    Ok(())
                });
                transport.expect_set_close_reason().return_once(drop);
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

/// Makes sure that `connect` fails immediately if transport establishment
/// failed.
#[wasm_bindgen_test]
async fn could_not_open_transport() {
    let session = WebSocketRpcSession::new(Rc::new(WebSocketRpcClient::new(
        Box::new(|url| {
            Box::pin(async move {
                let ws = WebSocketRpcTransport::new(url)
                    .await
                    .map_err(|e| tracerr::new!(e))?;
                Ok(Rc::new(ws) as Rc<dyn RpcTransport>)
            })
        }),
    )));

    let mut on_normal_close = session.on_normal_close().fuse();
    let mut on_reconnected = session.on_reconnected().fuse();
    let mut on_connection_loss = session.on_connection_loss().fuse();

    let connect_fut = Rc::clone(&session).connect(
        ConnectionInfo::from_str(
            "ws://localhost:55555/some/fake?token=endpoint",
        )
        .unwrap(),
    );

    // connect resolve with err
    timeout(100, connect_fut).await.unwrap().unwrap_err();

    // other callbacks should not fire
    futures::select! {
        _ = delay_for(100).fuse() => (),
        _ = on_normal_close => panic!("on_normal_close fired"),
        _ = on_connection_loss.next() => panic!("on_connection_loss fired"),
        _ = on_reconnected.next() => panic!("on_reconnected fired")
    };
}

/// Makes sure that `on_connection_loss` is fired when transport closes with
/// non-normal close and reconnect works as expected.
#[wasm_bindgen_test]
async fn reconnect_after_transport_abnormal_close() {
    let commands_sent = Rc::new(RefCell::new(Vec::new()));

    let commands_sent_clone = Rc::clone(&commands_sent);
    let session = WebSocketRpcSession::new(Rc::new(WebSocketRpcClient::new(
        Box::new(move |_| {
            let commands_sent_clone = Rc::clone(&commands_sent_clone);
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
                let commands_sent = Rc::clone(&commands_sent_clone);
                transport.expect_send().returning_st(move |msg| {
                    commands_sent.borrow_mut().push(msg.clone());
                    Ok(())
                });
                transport.expect_set_close_reason().return_once(drop);
                transport.expect_on_state_change().return_once_st(move || {
                    Box::pin(
                        stream::once(future::ready(TransportState::Open))
                            .chain(stream::once(async {
                                delay_for(20).await;
                                TransportState::Closed(CloseMsg::Abnormal(999))
                            })),
                    )
                });
                let transport = Rc::new(transport);
                Ok(transport as Rc<dyn RpcTransport>)
            })
        }),
    )));

    let mut on_normal_close = session.on_normal_close().fuse();
    let mut on_reconnected = session.on_reconnected().fuse();
    let mut on_connection_loss = session.on_connection_loss().fuse();

    let connect_fut = Rc::clone(&session)
        .connect(ConnectionInfo::from_str(TEST_ROOM_URL).unwrap());
    timeout(100, connect_fut).await.unwrap().unwrap();

    // on_connection_loss fires
    futures::select! {
        _ = delay_for(100).fuse() => panic!("on_connection_loss should fire"),
        _ = on_normal_close => panic!("on_normal_close fired"),
        _ = on_connection_loss.next() => (),
        _ = on_reconnected.next() => panic!("on_reconnected fired")
    };

    // successful reconnect after connection loss
    Rc::clone(&session).reconnect().await.unwrap();
    on_reconnected.select_next_some().await;

    drop(session);
    assert_eq!(
        *commands_sent.borrow(),
        vec![
            // connect
            ClientMsg::Command {
                room_id: "room_id".into(),
                command: Command::JoinRoom {
                    member_id: "member_id".into(),
                    credential: "token".into(),
                }
            },
            // reconnect
            ClientMsg::Command {
                room_id: "room_id".into(),
                command: Command::JoinRoom {
                    member_id: "member_id".into(),
                    credential: "token".into(),
                }
            }
        ]
    );
}

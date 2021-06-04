//! Tests for [`medea_jason::rpc::RpcClient`].

mod heartbeat;
mod reconnect_handle;
mod rpc_session;
mod websocket;

use std::{collections::HashMap, rc::Rc};

use futures::{
    channel::{mpsc, oneshot},
    future::{self},
    stream,
    stream::LocalBoxStream,
    StreamExt as _,
};
use medea_client_api_proto::{
    ClientMsg, CloseReason, Command, Event, PeerId, RpcSettings, ServerMsg,
};
use medea_jason::{
    platform::{MockRpcTransport, RpcTransport, TransportState},
    rpc::{ClientDisconnect, CloseMsg, RpcEvent, WebSocketRpcClient},
};
use wasm_bindgen_futures::spawn_local;
use wasm_bindgen_test::*;

use crate::{delay_for, join_room_url, timeout};

wasm_bindgen_test_configure!(run_in_browser);

/// [`ServerMsg::RpcSettings`] that can be used in tests.
pub const RPC_SETTINGS: ServerMsg = ServerMsg::RpcSettings(RpcSettings {
    idle_timeout_ms: 5_000,
    ping_interval_ms: 2_000,
});

/// Creates [`WebSocketRpcClient`] with the provided [`MockRpcTransport`].
fn new_client(transport: Rc<MockRpcTransport>) -> Rc<WebSocketRpcClient> {
    Rc::new(WebSocketRpcClient::new(Box::new(move |_| {
        Box::pin(future::ok(transport.clone() as Rc<dyn RpcTransport>))
    })))
}

/// Returns result for [`RpcTransport::on_message`] with [`LocalBoxStream`],
/// which will only send [`ServerMsg::RpcSettings`] with the provided
/// [`RpcSettings`].
fn on_message_mock(
    settings: RpcSettings,
) -> LocalBoxStream<'static, ServerMsg> {
    stream::once(async move { ServerMsg::RpcSettings(settings) })
        .chain(stream::pending())
        .boxed()
}

/// Tests [`WebSocketRpcClient::subscribe`] function.
///
/// # Algorithm
///
/// 1. Connect [`WebSocketRpcClient`] with [`MockRpcTransport`].
///
/// 2. Subscribe to [`Event`]s with [`WebSocketRpcClient::subscribe`].
///
/// 3. Send [`Event`] with [`MockRpcTransport`].
///
/// 4. Check that subscriber from step 2 receives this [`Event`].
#[wasm_bindgen_test]
async fn message_received_from_transport_is_transmitted_to_sub() {
    const SRV_EVENT: Event = Event::PeersRemoved {
        peer_ids: Vec::new(),
    };

    let ws = Rc::new(WebSocketRpcClient::new(Box::new(|_| {
        let mut transport = MockRpcTransport::new();
        transport.expect_on_state_change().return_once(|| {
            stream::once(async { TransportState::Open }).boxed()
        });
        transport.expect_on_message().returning(|| {
            stream::iter(vec![
                ServerMsg::RpcSettings(RpcSettings {
                    idle_timeout_ms: 10_000,
                    ping_interval_ms: 10_000,
                }),
                ServerMsg::Event {
                    room_id: "".into(),
                    event: SRV_EVENT,
                },
            ])
            .boxed()
        });
        transport.expect_send().returning(|_| Ok(()));
        transport.expect_set_close_reason().return_const(());

        Box::pin(future::ok(Rc::new(transport) as Rc<dyn RpcTransport>))
    })));

    let mut stream = ws.subscribe();
    ws.clone().connect(join_room_url()).await.unwrap();

    assert_eq!(
        stream.next().await.unwrap(),
        RpcEvent::Event {
            room_id: "".into(),
            event: SRV_EVENT
        }
    );
}

/// Tests that [`RpcTransport`] will be dropped when [`WebSocketRpcClient`] was
/// dropped.
///
/// # Algorithm
///
/// 1. Create [`WebSocketRpcClient`] with [`MockRpcTransport`] [`Rc`].
///
/// 2. Drop [`WebSocketRpcClient`].
///
/// 3. Check that [`MockRpcTransport`]'s [`Rc`] now have only 1
/// [`Rc::strong_count`].
#[wasm_bindgen_test]
async fn transport_is_dropped_when_client_is_dropped() {
    let mut transport = MockRpcTransport::new();
    transport.expect_send().returning(|_| Ok(()));
    transport.expect_set_close_reason().return_const(());
    transport
        .expect_on_state_change()
        .return_once(|| stream::once(async { TransportState::Open }).boxed());
    transport.expect_on_message().returning(|| {
        on_message_mock(RpcSettings {
            idle_timeout_ms: 10_000,
            ping_interval_ms: 500,
        })
    });
    let rpc_transport = Rc::new(transport);

    let ws = new_client(rpc_transport.clone());
    ws.clone().connect(join_room_url()).await.unwrap();
    ws.set_close_reason(ClientDisconnect::RoomClosed);
    drop(ws);
    delay_for(100).await;
    assert_eq!(Rc::strong_count(&rpc_transport), 1);
}

/// Tests [`WebSocketRpcClient::send_command`] function.
///
/// # Algorithm
///
/// 1. Connect [`WebSocketRpcClient`] with [`MockRpcTransport`].
///
/// 2. Subscribe to [`ClientMsg`]s which [`WebSocketRpcClient`] will send.
///
/// 3. Send [`ClientMsg`] with [`WebSocketRpcClient::send_command`].
///
/// 4. Check that this message received by [`MockRpcTransport`].
#[wasm_bindgen_test]
async fn send_goes_to_transport() {
    let mut transport = MockRpcTransport::new();
    let (on_send_tx, mut on_send_rx) = mpsc::unbounded();
    transport
        .expect_on_state_change()
        .return_once(|| stream::once(async { TransportState::Open }).boxed());
    transport.expect_on_message().returning(|| {
        on_message_mock(RpcSettings {
            idle_timeout_ms: 10_000,
            ping_interval_ms: 500,
        })
    });
    transport.expect_send().returning(move |e| {
        on_send_tx.unbounded_send(e.clone()).unwrap();
        Ok(())
    });
    transport.expect_set_close_reason().return_const(());

    let ws = new_client(Rc::new(transport));
    ws.clone().connect(join_room_url()).await.unwrap();
    let (test_tx, test_rx) = oneshot::channel();
    let test_peer_id = PeerId(9999);
    let test_sdp_offer = "Hello world!".to_string();
    let test_cmd = Command::MakeSdpOffer {
        peer_id: test_peer_id.clone(),
        sdp_offer: test_sdp_offer.clone(),
        mids: HashMap::new(),
        transceivers_statuses: HashMap::new(),
    };

    spawn_local(async move {
        while let Some(msg) = on_send_rx.next().await {
            match msg {
                ClientMsg::Command { command, .. } => match command {
                    Command::MakeSdpOffer {
                        peer_id,
                        sdp_offer,
                        mids: _,
                        transceivers_statuses: _,
                    } => {
                        assert_eq!(peer_id, test_peer_id);
                        assert_eq!(sdp_offer, test_sdp_offer);
                        test_tx.send(()).unwrap();
                        break;
                    }
                    _ => (),
                },
                _ => (),
            }
        }
    });

    ws.send_command("".into(), test_cmd);

    timeout(1000, test_rx).await.unwrap().unwrap();
}

/// Tests for [`WebSocketRpcClient::on_close`].
mod on_close {
    use super::*;

    /// Returns [`WebSocketRpcClient`] which will be resolved
    /// [`WebSocketRpcClient::on_close`] [`Future`] with provided
    /// [`CloseMsg`].
    async fn get_client(close_msg: CloseMsg) -> Rc<WebSocketRpcClient> {
        let mut transport = MockRpcTransport::new();
        transport.expect_on_state_change().return_once(move || {
            stream::iter(vec![
                TransportState::Open,
                TransportState::Closed(close_msg),
            ])
            .boxed()
        });
        transport.expect_on_message().returning(|| {
            on_message_mock(RpcSettings {
                idle_timeout_ms: 10_000,
                ping_interval_ms: 500,
            })
        });
        transport.expect_send().returning(|_| Ok(()));
        transport.expect_set_close_reason().return_const(());

        let ws = new_client(Rc::new(transport));
        ws.clone().connect(join_room_url()).await.unwrap();

        ws
    }

    /// Tests that [`WebSocketRpcClient::on_close`]'s [`Future`] resolves on
    /// normal closing.
    ///
    /// # Algorithm
    ///
    /// 1. Mock [`WebSocketRpcTransport::on_close`] to return
    ///    [`CloseReason::Finished`] with `1000` code.
    ///
    /// 2. Wait for [`WebSocketRpcTransport::on_close`] resolving.
    ///
    /// 3. Check that [`medea_jason::rpc::CloseReason`] returned from this
    ///    [`Future`] is [`rpc::CloseReason::ByServer`] with
    ///    [`CloseReason::Finished`] as reason.
    #[wasm_bindgen_test]
    async fn resolve_on_normal_closing() {
        let ws =
            get_client(CloseMsg::Normal(1000, CloseReason::Finished)).await;

        assert_eq!(
            ws.on_normal_close().await.unwrap(),
            medea_jason::rpc::CloseReason::ByServer(CloseReason::Finished)
        );
    }

    /// Tests that [`WebSocketRpcClient::on_close`]'s [`Future`] don't resolves
    /// on [`CloseReason::Reconnected`].
    ///
    /// # Algorithm
    ///
    /// 1. Mock [`WebSocketRpcTransport::on_close`] to return
    ///    [`CloseReason::Reconnected`] with `1000` code.
    ///
    /// 2. Wait `500ms` for [`WebSocketRpcTransport::on_close`] [`Future`]. If
    ///    in this time interval this [`Future`] wasn't resolved then test
    ///    considered passed.
    #[wasm_bindgen_test]
    async fn dont_resolve_on_reconnected_reason() {
        let ws =
            get_client(CloseMsg::Normal(1000, CloseReason::Reconnected)).await;

        timeout(500, ws.on_normal_close()).await.unwrap_err();
    }

    /// Tests that [`WebSocketRpcClient::on_close`]'s [`Future`] don't resolves
    /// on [`CloseMsg::Abnormal`].
    ///
    /// # Algorithm
    ///
    /// 1. Mock [`WebSocketRpcTransport::on_close`] to return
    ///    [`CloseMsg::Abnormal`] with `1500` code.
    ///
    /// 2. Wait `500ms` for [`WebSocketRpcTransport::on_close`] [`Future`]. If
    ///    in this time interval this [`Future`] wasn't resolved then test
    ///    considered passed.
    #[wasm_bindgen_test]
    async fn dont_resolve_on_abnormal_close() {
        let ws = get_client(CloseMsg::Abnormal(1500)).await;

        timeout(500, ws.on_normal_close()).await.unwrap_err();
    }
}

/// Tests which checks that when [`WebSocketRpcClient`] is dropped the right
/// close reason is provided to [`RpcTransport`].
mod transport_close_reason_on_drop {
    use super::*;

    /// Returns [`WebSocketRpcClient`] and [`oneshot::Receiver`] which will be
    /// resolved with [`RpcTransport`]'s close reason
    /// ([`ClientDisconnect`]).
    async fn get_client(
    ) -> (Rc<WebSocketRpcClient>, oneshot::Receiver<ClientDisconnect>) {
        let mut transport = MockRpcTransport::new();
        transport.expect_on_state_change().return_once(|| {
            stream::once(async { TransportState::Open }).boxed()
        });
        transport.expect_on_message().returning(|| {
            on_message_mock(RpcSettings {
                idle_timeout_ms: 10000,
                ping_interval_ms: 500,
            })
        });
        transport.expect_send().return_once(|_| Ok(()));
        let (test_tx, test_rx) = oneshot::channel();
        transport
            .expect_set_close_reason()
            .return_once(move |reason| {
                test_tx.send(reason).unwrap();
            });

        let ws = new_client(Rc::new(transport));
        ws.clone().connect(join_room_url()).await.unwrap();

        (ws, test_rx)
    }

    /// Tests that [`RpcClient`] sets right [`ClientDisconnect`] close reason on
    /// UNexpected drop.
    ///
    /// # Algorithm
    ///
    /// 1. Mock [`RpcTransport::set_close_reason`].
    ///
    /// 2. Drop [`WebSocketRpcClient`].
    ///
    /// 3. Check that close reason provided
    ///    into [`RpcTransport::set_close_reason`]
    ///    is [`ClientDisconnect::RpcClientUnexpectedlyDropped`].
    #[wasm_bindgen_test]
    async fn sets_default_close_reason_on_drop() {
        let (ws, test_rx) = get_client().await;

        drop(ws);

        let close_reason = test_rx.await.unwrap();
        assert_eq!(
            close_reason,
            ClientDisconnect::RpcClientUnexpectedlyDropped,
            "RpcClient sets RpcTransport close reason '{:?}' instead of \
             'RpcClientUnexpectedlyDropped'.",
            close_reason,
        );
    }

    /// Tests that [`RpcClient`] sets right [`ClientDisconnect`] close reason on
    /// expected drop.
    ///
    /// # Algorithm
    ///
    /// 1. Mock [`RpcTransport::set_close_reason`].
    ///
    /// 2. Set [`ClientDisconnect::RoomClosed`] close reason and drop
    ///    [`WebSocketRpcClient`].
    ///
    /// 3. Check that close reason provided
    ///    into [`RpcTransport::set_close_reason`]
    ///    is [`ClientDisconnect::RoomClosed`].
    #[wasm_bindgen_test]
    async fn sets_provided_close_reason_on_drop() {
        let (ws, test_rx) = get_client().await;

        ws.set_close_reason(ClientDisconnect::RoomClosed);
        drop(ws);

        let close_reason = test_rx.await.unwrap();
        assert_eq!(
            close_reason,
            ClientDisconnect::RoomClosed,
            "RpcClient sets RpcTransport close reason '{:?}' instead of \
             'RoomClosed'.",
            close_reason,
        );
    }
}

/// Tests for [`RpcClient::connect`].
mod connect {
    use medea_client_api_proto::RpcSettings;

    use crate::delay_for;

    use super::*;

    /// Tests that new connection will be created if [`RpcClient`] is in
    /// [`State::Closed`].
    ///
    /// # Algorithm
    ///
    /// 1. Create new [`WebSocketRpcClient`].
    ///
    /// 2. Call [`WebSocketRpcClient::connect`] and check that it successfully
    ///    resolved.
    #[wasm_bindgen_test]
    async fn closed() {
        let (test_tx, mut test_rx) = mpsc::unbounded();
        let ws = Rc::new(WebSocketRpcClient::new(Box::new(move |_| {
            test_tx.unbounded_send(()).unwrap();
            let mut transport = MockRpcTransport::new();
            transport.expect_on_message().times(3).returning(|| {
                on_message_mock(RpcSettings {
                    idle_timeout_ms: 3_000,
                    ping_interval_ms: 3_000,
                })
            });
            transport.expect_send().return_once(|_| Ok(()));
            transport.expect_set_close_reason().return_once(drop);
            transport.expect_on_state_change().return_once(|| {
                stream::once(async { TransportState::Open }).boxed()
            });
            let transport = Rc::new(transport);
            Box::pin(future::ok(transport as Rc<dyn RpcTransport>))
        })));
        ws.clone().connect(join_room_url()).await.unwrap();

        timeout(500, test_rx.next()).await.unwrap().unwrap();
    }

    /// Tests that new connection try will be not started if
    /// [`WebSocketRpcClient`] is already in [`State::Connecting`].
    ///
    /// # Algorithm
    ///
    /// 1. Create new [`WebSocketRpcClient`] with [`RpcTransport`] factory which
    ///    will be resolved after 500 milliseconds.
    ///
    /// 2. Call [`WebSocketRpcClient::connect`] in [`spawn_local`].
    ///
    /// 3. Simultaneously with it call another [`WebSocketRpcClient::connect`].
    ///
    /// 4. Check that only one [`RpcTransport`] was created.
    #[wasm_bindgen_test]
    async fn connecting() {
        let mut connecting_count: i32 = 0;
        let ws = Rc::new(WebSocketRpcClient::new(Box::new(move |_| {
            Box::pin(async move {
                let mut transport = MockRpcTransport::new();
                transport.expect_on_message().times(3).returning(|| {
                    on_message_mock(RpcSettings {
                        idle_timeout_ms: 3_000,
                        ping_interval_ms: 3_000,
                    })
                });
                transport.expect_send().return_once(|_| Ok(()));
                transport.expect_set_close_reason().return_once(drop);
                transport.expect_on_state_change().return_once(|| {
                    stream::once(async { TransportState::Open }).boxed()
                });
                let transport = Rc::new(transport);
                connecting_count += 1;
                if connecting_count > 1 {
                    unreachable!("New connection try was performed!");
                } else {
                    delay_for(500).await;
                    Ok(Rc::clone(&transport) as Rc<dyn RpcTransport>)
                }
            })
        })));
        let first_connect_fut = ws.clone().connect(join_room_url());
        spawn_local(async move {
            first_connect_fut.await.unwrap();
        });

        timeout(1000, ws.connect(join_room_url()))
            .await
            .unwrap()
            .unwrap();
    }

    /// Tests that [`WebSocketRpcClient::connect`] will be instantly resolved
    /// if [`State`] is already [`State::Open`].
    ///
    /// # Algorithm
    ///
    /// 1. Normally connect [`WebSocketRpcClient`].
    ///
    /// 2. Call [`WebSocketRpcClient::connect`] again.
    ///
    /// 3. Check that only one [`RpcTransport`] was created.
    #[wasm_bindgen_test]
    async fn open() {
        let mut connection_count = 0;
        let ws = Rc::new(WebSocketRpcClient::new(Box::new(move |_| {
            Box::pin(async move {
                connection_count += 1;
                if connection_count > 1 {
                    unreachable!("Only one connection should be performed!");
                }
                let mut transport = MockRpcTransport::new();
                transport.expect_on_message().times(3).returning(|| {
                    on_message_mock(RpcSettings {
                        idle_timeout_ms: 3_000,
                        ping_interval_ms: 3_000,
                    })
                });
                transport.expect_send().return_once(|_| Ok(()));
                transport.expect_set_close_reason().return_once(drop);
                transport.expect_on_state_change().return_once(|| {
                    stream::once(async { TransportState::Open }).boxed()
                });
                let transport = Rc::new(transport);
                Ok(transport as Rc<dyn RpcTransport>)
            })
        })));
        ws.clone().connect(join_room_url()).await.unwrap();

        timeout(50, ws.connect(join_room_url()))
            .await
            .unwrap()
            .unwrap();
    }
}

/// Tests for [`RpcClient::on_connection_loss`].
mod on_connection_loss {

    use medea_client_api_proto::RpcSettings;

    use super::*;

    async fn helper(
        idle_timeout_ms: Option<u32>,
        ping_interval_ms: Option<u32>,
        transport_changes: Option<TransportState>,
    ) -> Rc<WebSocketRpcClient> {
        let ws = Rc::new(WebSocketRpcClient::new(Box::new(move |_| {
            Box::pin(async move {
                let mut transport = MockRpcTransport::new();
                transport.expect_on_message().times(3).returning(move || {
                    on_message_mock(RpcSettings {
                        idle_timeout_ms: idle_timeout_ms
                            .unwrap_or(u32::max_value()),
                        ping_interval_ms: ping_interval_ms
                            .unwrap_or(u32::max_value()),
                    })
                });
                transport.expect_set_close_reason().return_once(drop);

                transport.expect_on_state_change().return_once(move || {
                    stream::once(async move { TransportState::Open })
                        .chain(
                            transport_changes
                                .map(|v| stream::once(async move { v }).boxed())
                                .unwrap_or(stream::empty().boxed()),
                        )
                        .chain(stream::pending())
                        .boxed()
                });
                transport.expect_send().returning(|_| Ok(()));
                let transport = Rc::new(transport);
                Ok(transport as Rc<dyn RpcTransport>)
            })
        })));
        ws.clone().connect(join_room_url()).await.unwrap();

        ws
    }

    /// [`WebSocketRpcClient::on_connection_loss`] procs when no pings received.
    #[wasm_bindgen_test]
    async fn on_reconnected() {
        let ws = helper(Some(100), Some(10), None).await;

        timeout(90, ws.on_connection_loss().next())
            .await
            .unwrap_err();

        timeout(150, ws.on_connection_loss().next())
            .await
            .unwrap()
            .unwrap();

        timeout(100, ws.on_normal_close()).await.unwrap_err();
    }

    /// 1. `on_connection_loss` procs and `on_normal_close` doesnt on ws close
    /// with `CloseMsg::Abnormal`.
    /// 2. `on_connection_loss` procs and `on_normal_close` doesnt on ws close
    /// with `CloseMsg::Normal(CloseReason::Idle)`.
    /// 3. Neither `on_connection_loss` nor `on_normal_close` procs on ws
    /// close with `CloseMsg::Normal(CloseReason::Reconnected)`.
    /// 4. `on_connection_loss` doesnt proc, and `on_normal_close` does on ws
    /// close with other messages.
    #[wasm_bindgen_test]
    async fn connection_loss() {
        async fn connection_loss_helper(
            transport_state: TransportState,
            should_loss: bool,
            should_normal_close: bool,
        ) {
            let ws = helper(None, None, Some(transport_state)).await;

            let on_normal_close = ws.on_normal_close();
            let mut on_connection_loss = ws.on_connection_loss();

            let on_normal_close = timeout(100, on_normal_close).await;
            if should_normal_close {
                on_normal_close.unwrap().unwrap();
            } else {
                on_normal_close.unwrap_err();
            }

            let on_connection_loss =
                timeout(100, on_connection_loss.next()).await;
            if should_loss {
                on_connection_loss.unwrap();
            } else {
                on_connection_loss.unwrap_err();
            }
        }

        connection_loss_helper(
            TransportState::Closed(CloseMsg::Abnormal(1006)),
            true,
            false,
        )
        .await;
        connection_loss_helper(
            TransportState::Closed(CloseMsg::Normal(1000, CloseReason::Idle)),
            true,
            false,
        )
        .await;
        connection_loss_helper(
            TransportState::Closed(CloseMsg::Normal(
                1000,
                CloseReason::Reconnected,
            )),
            false,
            false,
        )
        .await;

        // other messages
        connection_loss_helper(
            TransportState::Closed(CloseMsg::Normal(
                1000,
                CloseReason::Finished,
            )),
            false,
            true,
        )
        .await;
        connection_loss_helper(
            TransportState::Closed(CloseMsg::Normal(
                1000,
                CloseReason::Evicted,
            )),
            false,
            true,
        )
        .await;
        connection_loss_helper(
            TransportState::Closed(CloseMsg::Normal(
                1000,
                CloseReason::InternalError,
            )),
            false,
            true,
        )
        .await;
        connection_loss_helper(
            TransportState::Closed(CloseMsg::Normal(
                1000,
                CloseReason::Rejected,
            )),
            false,
            true,
        )
        .await;

        // reminder to extend test if new reason is added
        match CloseReason::Finished {
            CloseReason::Finished => {}
            CloseReason::Reconnected => {}
            CloseReason::Idle => {}
            CloseReason::Rejected => {}
            CloseReason::InternalError => {}
            CloseReason::Evicted => {}
        }
    }
}

/// Tests for the [`RpcClient::on_reconnected`] function.
// TODO: this tests should be implemented for the RpcSession!
#[cfg(feature = "disabled")]
mod on_reconnected {

    use medea_reactive::ObservableCell;

    use crate::yield_now;

    use super::*;

    /// Checks that [`RpcClient::on_reconnected`] doesn't fires on
    /// first [`RpcClient`] connection.
    #[wasm_bindgen_test]
    async fn doesnt_fires_on_first_connection() {
        let ws = Rc::new(WebSocketRpcClient::new(Box::new(move |_| {
            Box::pin(async move {
                let mut transport = MockRpcTransport::new();
                transport.expect_on_message().times(3).returning(|| {
                    on_message_mock(RpcSettings {
                        idle_timeout_ms: 5_000,
                        ping_interval_ms: 2_000,
                    })
                });
                transport.expect_send().return_once(|_| Ok(()));
                transport.expect_set_close_reason().return_once(drop);
                transport.expect_on_state_change().return_once(|| {
                    stream::once(async { TransportState::Open }).boxed()
                });

                Ok(Rc::new(transport) as Rc<dyn RpcTransport>)
            })
        })));

        let mut on_reconnected_stream = ws.on_reconnected();
        ws.clone().connect(join_room_url()).await.unwrap();
        timeout(10, on_reconnected_stream.next()).await.unwrap_err();
    }

    /// Checks that [`RpcClient::on_reconnected`] will fire on real
    /// connection restore.
    #[wasm_bindgen_test]
    async fn fires_on_reconnection() {
        let on_message_mock =
            Rc::new(ObservableCell::new(ServerMsg::RpcSettings(RpcSettings {
                idle_timeout_ms: 5_000,
                ping_interval_ms: 2_000,
            })));
        let on_state_change_mock =
            Rc::new(ObservableCell::new(TransportState::Open));

        let on_close_mock_clone = on_state_change_mock.clone();
        let on_message_mock_clone = on_message_mock.clone();

        let ws = Rc::new(WebSocketRpcClient::new(Box::new(move |_| {
            let messages_mock = on_message_mock_clone.clone();
            let on_close_mock = on_close_mock_clone.clone();
            Box::pin(async move {
                let mut transport = MockRpcTransport::new();
                transport
                    .expect_on_message()
                    .times(3)
                    .returning_st(move || messages_mock.subscribe());
                transport.expect_send().return_once(|_| Ok(()));
                transport.expect_set_close_reason().return_once(drop);
                transport
                    .expect_on_state_change()
                    .return_once_st(move || on_close_mock.subscribe());
                let transport = Rc::new(transport);
                Ok(transport as Rc<dyn RpcTransport>)
            })
        })));

        let mut on_reconnected_stream = ws.on_reconnected();
        ws.clone().connect(join_room_url()).await.unwrap();

        on_state_change_mock
            .set(TransportState::Closed(CloseMsg::Abnormal(1006)));
        // Release async runtime so State::Closed can be processed.
        yield_now().await;

        on_state_change_mock.set(TransportState::Open);
        on_message_mock.set(ServerMsg::RpcSettings(RpcSettings {
            idle_timeout_ms: 5_000,
            ping_interval_ms: 2_000,
        }));

        ws.connect(join_room_url()).await.unwrap();
        assert!(on_reconnected_stream.next().await.is_some());
    }
}

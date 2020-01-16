//! Tests for [`medea_jason::rpc::RpcClient`].

mod backoff_delayer;
mod heartbeat;
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
use medea_jason::rpc::{
    ClientDisconnect, CloseMsg, ClosedStateReason, MockRpcTransport, RpcClient,
    RpcTransport, State, WebSocketRpcClient,
};
use wasm_bindgen_futures::spawn_local;
use wasm_bindgen_test::*;

use crate::{await_with_timeout, resolve_after};

wasm_bindgen_test_configure!(run_in_browser);

/// Creates [`WebSocketRpcClient`] with the provided [`MockRpcTransport`].
fn new_client(transport: Rc<MockRpcTransport>) -> WebSocketRpcClient {
    WebSocketRpcClient::new(Box::new(move |_| {
        Box::pin(future::ok(transport.clone() as Rc<dyn RpcTransport>))
    }))
}

/// Returns result for [`RpcTransport::on_message`] with [`LocalBoxStream`],
/// which will only send [`ServerMsg::RpcSettings`] with the provided
/// [`RpcSettings`].
fn on_message_mock(
    settings: RpcSettings,
) -> LocalBoxStream<'static, ServerMsg> {
    stream::once(async move { ServerMsg::RpcSettings(settings) }).boxed()
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

    let ws = WebSocketRpcClient::new(Box::new(|_| {
        let mut transport = MockRpcTransport::new();
        transport
            .expect_on_state_change()
            .return_once(|| stream::once(async { State::Open }).boxed());
        transport.expect_on_message().returning(|| {
            let (tx, rx) = mpsc::unbounded();
            tx.unbounded_send(ServerMsg::RpcSettings(RpcSettings {
                idle_timeout_ms: 10_000,
                ping_interval_ms: 10_000,
            }))
            .unwrap();
            tx.unbounded_send(ServerMsg::Event(SRV_EVENT)).unwrap();
            rx.boxed()
        });
        transport.expect_send().returning(|_| Ok(()));
        transport.expect_set_close_reason().return_const(());

        Box::pin(future::ok(Rc::new(transport) as Rc<dyn RpcTransport>))
    }));

    let mut stream = ws.subscribe();
    ws.connect(String::new()).await.unwrap();
    assert_eq!(stream.next().await.unwrap(), SRV_EVENT);
}

/// Tests [`WebSocketRpcClient::unsub`] function.
///
/// # Algorithm
///
/// 1. Subscribe to [`Event`]s with [`WebSocketRpcClient::subscribe`].
///
/// 2. Call [`WebSocketRpcClient::unsub`].
///
/// 3. Wait for `None` received from [`WebSocketRpcClient::subscribe`]'s
/// `Stream`.
#[wasm_bindgen_test]
async fn unsub_drops_subs() {
    let ws = new_client(Rc::new(MockRpcTransport::new()));
    let (test_tx, test_rx) = oneshot::channel();
    let mut subscriber_stream = ws.subscribe();
    spawn_local(async move {
        loop {
            match subscriber_stream.next().await {
                Some(_) => (),
                None => {
                    test_tx.send(()).unwrap();
                    break;
                }
            }
        }
    });
    ws.unsub();

    await_with_timeout(Box::pin(test_rx), 1000)
        .await
        .unwrap()
        .unwrap();
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
        .return_once(|| stream::once(async { State::Open }).boxed());
    transport.expect_on_message().returning(|| {
        on_message_mock(RpcSettings {
            idle_timeout_ms: 10_000,
            ping_interval_ms: 500,
        })
    });
    let rpc_transport = Rc::new(transport);

    let ws = new_client(rpc_transport.clone());
    ws.connect(String::new()).await.unwrap();
    ws.set_close_reason(ClientDisconnect::RoomClosed);
    drop(ws);
    resolve_after(100).await.unwrap();
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
    // We don't use mockall's '.withf' instead of channel because in case
    // of '.withf' usage we should move 'test_tx' to all '.withf' closures
    // (but we can't do this).
    let (on_send_tx, mut on_send_rx) = mpsc::unbounded();
    transport
        .expect_on_state_change()
        .return_once(|| stream::once(async { State::Open }).boxed());
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
    ws.connect(String::new()).await.unwrap();
    let (test_tx, test_rx) = oneshot::channel();
    let test_peer_id = PeerId(9999);
    let test_sdp_offer = "Hello world!".to_string();
    let test_cmd = Command::MakeSdpOffer {
        peer_id: test_peer_id.clone(),
        sdp_offer: test_sdp_offer.clone(),
        mids: HashMap::new(),
    };

    spawn_local(async move {
        while let Some(msg) = on_send_rx.next().await {
            match msg {
                ClientMsg::Command(cmd) => match cmd {
                    Command::MakeSdpOffer {
                        peer_id,
                        sdp_offer,
                        mids: _,
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

    ws.send_command(test_cmd);

    await_with_timeout(Box::pin(test_rx), 1000)
        .await
        .unwrap()
        .unwrap();
}

/// Tests for [`WebSocketRpcClient::on_close`].
mod on_close {
    use super::*;

    /// Returns [`WebSocketRpcClient`] which will be resolved
    /// [`WebSocketRpcClient::on_close`] [`Future`] with provided
    /// [`CloseMsg`].
    async fn get_client(close_msg: CloseMsg) -> WebSocketRpcClient {
        let mut transport = MockRpcTransport::new();
        transport.expect_on_state_change().return_once(|| {
            let (tx, rx) = mpsc::unbounded();
            tx.unbounded_send(State::Open).unwrap();
            tx.unbounded_send(State::Closed(
                ClosedStateReason::ConnectionLost(close_msg),
            ))
            .unwrap();
            Box::pin(rx)
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
        ws.connect(String::new()).await.unwrap();

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

        await_with_timeout(Box::pin(ws.on_normal_close()), 500)
            .await
            .unwrap_err();
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

        await_with_timeout(Box::pin(ws.on_normal_close()), 500)
            .await
            .unwrap_err();
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
    ) -> (WebSocketRpcClient, oneshot::Receiver<ClientDisconnect>) {
        let mut transport = MockRpcTransport::new();
        transport
            .expect_on_state_change()
            .return_once(|| stream::once(async { State::Open }).boxed());
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
        ws.connect(String::new()).await.unwrap();

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
    use medea_jason::rpc::State;

    use crate::resolve_after;

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
        let ws = WebSocketRpcClient::new(Box::new(move |_| {
            test_tx.unbounded_send(()).unwrap();
            let mut transport = MockRpcTransport::new();
            transport.expect_on_message().times(3).returning(|| {
                on_message_mock(RpcSettings {
                    idle_timeout_ms: 3_000,
                    ping_interval_ms: 3_000,
                })
            });
            transport.expect_send().return_once(|_| Ok(()));
            transport.expect_set_close_reason().return_once(|_| ());
            transport
                .expect_on_state_change()
                .return_once(|| stream::once(async { State::Open }).boxed());
            let transport = Rc::new(transport);
            Box::pin(future::ok(transport as Rc<dyn RpcTransport>))
        }));
        ws.connect(String::new()).await.unwrap();

        await_with_timeout(Box::pin(test_rx.next()), 500)
            .await
            .unwrap()
            .unwrap();
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
        let ws = WebSocketRpcClient::new(Box::new(move |_| {
            Box::pin(async move {
                let mut transport = MockRpcTransport::new();
                transport.expect_on_message().times(3).returning(|| {
                    on_message_mock(RpcSettings {
                        idle_timeout_ms: 3_000,
                        ping_interval_ms: 3_000,
                    })
                });
                transport.expect_send().return_once(|_| Ok(()));
                transport.expect_set_close_reason().return_once(|_| ());
                transport.expect_on_state_change().return_once(|| {
                    stream::once(async { State::Open }).boxed()
                });
                let transport = Rc::new(transport);
                connecting_count += 1;
                if connecting_count > 1 {
                    unreachable!("New connection try was performed!");
                } else {
                    resolve_after(500).await.unwrap();
                    Ok(Rc::clone(&transport) as Rc<dyn RpcTransport>)
                }
            })
        }));
        let first_connect_fut = ws.connect(String::new());
        spawn_local(async move {
            first_connect_fut.await.unwrap();
        });

        await_with_timeout(Box::pin(ws.connect(String::new())), 1000)
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
        let ws = WebSocketRpcClient::new(Box::new(move |_| {
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
                transport.expect_set_close_reason().return_once(|_| ());
                transport.expect_on_state_change().return_once(|| {
                    stream::once(async { State::Open }).boxed()
                });
                let transport = Rc::new(transport);
                Ok(transport as Rc<dyn RpcTransport>)
            })
        }));
        ws.connect(String::new()).await.unwrap();

        await_with_timeout(Box::pin(ws.connect(String::new())), 50)
            .await
            .unwrap()
            .unwrap();
    }
}

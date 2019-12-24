//! Tests for [`medea_jason::rpc::RpcClient`].

mod backoff_delayer;
mod heartbeat;
mod websocket;

use std::{
    collections::HashMap,
    rc::Rc,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use futures::{
    channel::{mpsc, oneshot},
    future::{self, Either},
    stream, FutureExt as _, StreamExt as _,
};
use medea_client_api_proto::{
    ClientMsg, CloseReason, Command, Event, PeerId, ServerMsg,
};
use medea_jason::rpc::{
    ClientDisconnect, CloseMsg, IdleTimeout, MockRpcTransport, PingInterval,
    RpcClient, WebSocketRpcClient,
};
use wasm_bindgen_futures::spawn_local;
use wasm_bindgen_test::*;

use crate::{await_with_timeout, resolve_after};

wasm_bindgen_test_configure!(run_in_browser);

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

    let mut transport = MockRpcTransport::new();
    transport.expect_on_message().returning(|| {
        Ok(stream::once(async { ServerMsg::Event(SRV_EVENT) }).boxed())
    });
    transport.expect_send().returning(|_| Ok(()));
    transport
        .expect_on_close()
        .return_once(|| Ok(stream::pending().boxed()));
    transport.expect_set_close_reason().return_const(());

    let ws = WebSocketRpcClient::new();
    ws.update_settings(
        IdleTimeout(Duration::from_secs(10).into()),
        PingInterval(Duration::from_millis(10).into()),
    );

    let mut stream = ws.subscribe();
    ws.connect(Rc::new(transport)).await.unwrap();
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
    let ws = WebSocketRpcClient::new();
    ws.update_settings(
        IdleTimeout(Duration::from_secs(10).into()),
        PingInterval(Duration::from_millis(500).into()),
    );
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
    transport
        .expect_on_message()
        .returning(|| Ok(stream::once(future::pending()).boxed()));
    transport
        .expect_on_close()
        .return_once(|| Ok(stream::once(future::pending()).boxed()));
    transport.expect_send().returning(|_| Ok(()));
    transport.expect_set_close_reason().return_const(());
    let rpc_transport = Rc::new(transport);

    let ws = WebSocketRpcClient::new();
    ws.update_settings(
        IdleTimeout(Duration::from_secs(10).into()),
        PingInterval(Duration::from_millis(500).into()),
    );
    ws.connect(rpc_transport.clone()).await.unwrap();
    ws.set_close_reason(ClientDisconnect::RoomClosed);
    std::mem::drop(ws);
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
        .expect_on_message()
        .times(2)
        .returning(|| Ok(stream::once(future::pending()).boxed()));
    transport
        .expect_on_close()
        .return_once(move || Ok(stream::pending().boxed()));
    transport.expect_send().returning(move |e| {
        on_send_tx.unbounded_send(e.clone()).unwrap();
        Ok(())
    });
    transport.expect_set_close_reason().return_const(());

    let ws = WebSocketRpcClient::new();
    ws.update_settings(
        IdleTimeout(Duration::from_secs(10).into()),
        PingInterval(Duration::from_millis(500).into()),
    );
    ws.connect(Rc::new(transport)).await.unwrap();
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
    async fn get_client(close_msg: CloseMsg) -> Rc<WebSocketRpcClient> {
        let mut transport = MockRpcTransport::new();
        transport
            .expect_on_message()
            .returning(|| Ok(stream::once(future::pending()).boxed()));
        transport.expect_send().returning(|_| Ok(()));
        transport
            .expect_reconnect()
            .return_once(|| future::pending().boxed());
        transport.expect_set_close_reason().return_const(());
        transport.expect_on_close().return_once(move || {
            Ok(stream::once(async move { close_msg }).boxed())
        });

        let ws = WebSocketRpcClient::new();
        ws.update_settings(
            IdleTimeout(Duration::from_secs(10).into()),
            PingInterval(Duration::from_millis(500).into()),
        );
        ws.connect(Rc::new(transport)).await.unwrap();

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
            ws.on_close().await.unwrap(),
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

        match future::select(
            Box::pin(ws.on_close()),
            Box::pin(resolve_after(500)),
        )
        .await
        {
            Either::Left((msg, _)) => {
                unreachable!(
                    "Some CloseMsg was unexpectedly thrown: {:?}.",
                    msg
                );
            }
            Either::Right(_) => (),
        }
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

        match future::select(
            Box::pin(ws.on_close()),
            Box::pin(resolve_after(500)),
        )
        .await
        {
            Either::Left((msg, _)) => {
                unreachable!(
                    "Some CloseMsg was unexpectedly thrown: {:?}.",
                    msg
                );
            }
            Either::Right(_) => (),
        }
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
        transport
            .expect_on_message()
            .times(2)
            .returning(|| Ok(stream::once(future::pending()).boxed()));
        transport.expect_send().return_once(|_| Ok(()));
        transport
            .expect_on_close()
            .return_once(|| Ok(stream::pending().boxed()));
        let (test_tx, test_rx) = oneshot::channel();
        transport
            .expect_set_close_reason()
            .return_once(move |reason| {
                test_tx.send(reason).unwrap();
            });

        let ws = WebSocketRpcClient::new();
        ws.update_settings(
            IdleTimeout(Duration::from_secs(10).into()),
            PingInterval(Duration::from_millis(500).into()),
        );
        ws.connect(Rc::new(transport)).await.unwrap();

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

        std::mem::drop(ws);

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
        std::mem::drop(ws);

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

/// Tests which checks that on abnormal [`RpcTransport`] close, [`RpcClient`]
/// tries to reconnect [`RpcTransport`].
mod reconnect {
    use medea_jason::{
        rpc::{ReconnectableRpcClient, State, TransportError},
        utils::JsDuration,
    };

    use crate::await_with_timeout;

    use super::*;

    /// Tests that [`WebSocketRpcClient`] will resolve
    /// [`RpcClient::on_connection_loss`] on abnormal connection loss.
    ///
    /// # Algorithm
    ///
    /// 1. Mock [`RpcTransport`] to throw [`CloseMsg::Abnormal`].
    ///
    /// 2. Wait for [`RpcClient::on_connection_loss`] resolving (with 600
    ///    milliseconds timeout).
    async fn on_abnormal_transport_close() {
        let mut transport = MockRpcTransport::new();
        let (on_close_tx, on_close_rx) = mpsc::unbounded();
        transport
            .expect_on_message()
            .times(2)
            .returning(|| Ok(stream::once(future::pending()).boxed()));
        transport.expect_send().returning(|_| Ok(()));
        transport.expect_set_close_reason().return_const(());
        transport
            .expect_on_close()
            .return_once(move || Ok(on_close_rx.boxed()));
        let ws = WebSocketRpcClient::new();
        ws.update_settings(
            IdleTimeout(Duration::from_millis(125).into()),
            PingInterval(Duration::from_millis(250).into()),
        );
        ws.connect(Rc::new(transport)).await.unwrap();

        on_close_tx
            .unbounded_send(CloseMsg::Abnormal(1500))
            .unwrap();

        await_with_timeout(Box::pin(ws.on_connection_loss().next()), 600)
            .await
            .unwrap()
            .unwrap();
    }

    /// Tests that [`RpcClient::reconnect_with_backoff`] calls
    /// [`RpcTransport::reconnect`] many times with some delay until
    /// reconnection is not successful.
    ///
    /// # Algorithm
    ///
    /// 1. Mock [`RpcTransport::reconnect`] and [`RpcTransport::on_close`].
    ///
    /// 2. Send [`CloseMsg::Abnormal`] to [`RpcTransport::on_close`] [`Stream`].
    ///
    /// 3. Wait for 3 calls of [`RpcTransport::reconnect`] (while this function
    ///    is not called 3 times, result of reconnection will be
    ///    [`TransportError::InitSocket`]).
    #[wasm_bindgen_test]
    async fn timeout() {
        let mut transport = MockRpcTransport::new();
        let (test_tx, mut test_rx) = mpsc::unbounded();
        let reconnection_count = AtomicU64::new(0);
        transport
            .expect_on_message()
            .returning(|| Ok(stream::once(future::pending()).boxed()));
        transport.expect_send().returning(|_| Ok(()));
        transport.expect_set_close_reason().return_const(());
        transport.expect_get_state().return_const(State::Closed);
        transport
            .expect_on_close()
            .return_once(|| Ok(stream::pending().boxed()));
        transport.expect_on_state_change().returning(move || {
            stream::once(async move { State::Open }).boxed()
        });

        transport.expect_reconnect().returning(move || {
            let current_reconnection_count =
                reconnection_count.load(Ordering::Relaxed);
            let count = current_reconnection_count + 1;
            reconnection_count.store(count, Ordering::Relaxed);
            if count >= 3 {
                test_tx.unbounded_send(()).unwrap();
                future::ok(()).boxed()
            } else {
                future::err(tracerr::new!(TransportError::InitSocket)).boxed()
            }
        });

        let ws = WebSocketRpcClient::new();
        ws.update_settings(
            IdleTimeout(Duration::from_secs(10).into()),
            PingInterval(Duration::from_millis(500).into()),
        );
        ws.connect(Rc::new(transport)).await.unwrap();
        spawn_local(async move {
            ws.reconnect_with_backoff(
                Duration::from_millis(500).into(),
                2.0,
                Duration::from_secs(10).into(),
            )
            .await;
        });

        const TIMEOUT_FOR_TWO_RECONNECTIONS: i32 = 3800;
        await_with_timeout(
            Box::pin(test_rx.next()),
            TIMEOUT_FOR_TWO_RECONNECTIONS,
        )
        .await
        .unwrap()
        .unwrap();
    }
}

/// Tests for mechanism of subscribing to the [`State`] of [`RpcTransport`]
/// when reconnection already started.
mod subscribe_to_state {
    use futures::{
        channel::mpsc, future, stream, FutureExt as _, SinkExt, StreamExt as _,
    };
    use medea_jason::rpc::{
        CloseMsg, MockRpcTransport, ReconnectableRpcClient, State,
    };

    use super::*;

    /// Tests that [`RpcClient`] will start reconnection if [`RpcTransport`]'s
    /// [`State`] is [`State::Closed`].
    ///
    /// # Algorithm
    ///
    /// 1. Mock [`RpcTransport`] to return [`State::Closed`] from
    ///    [`RpcTransport::get_state`].
    ///
    /// 2. Mock [`RpcTransport::on_close`] to return [`CloseMsg::Abnormal`].
    ///
    /// 3. Try to call [`RpcClient::reconnect`] and check that
    ///    [`RpcTransport::reconnect`]    was called one time.
    #[wasm_bindgen_test]
    async fn closed_transport_state() {
        let mut transport = MockRpcTransport::new();
        transport.expect_get_state().return_once(|| State::Closed);
        let (on_state_change_tx, on_state_change_rx) = mpsc::unbounded();
        transport
            .expect_on_state_change()
            .return_once(|| on_state_change_rx.boxed());
        transport.expect_on_close().return_once(|| {
            Ok(stream::once(async { CloseMsg::Abnormal(1500) }).boxed())
        });
        transport
            .expect_on_message()
            .times(3)
            .returning(|| Ok(stream::pending().boxed()));
        transport
            .expect_set_close_reason()
            .times(1)
            .returning(|_| ());
        let (test_tx, test_rx) = oneshot::channel();
        transport.expect_reconnect().return_once(move || {
            test_tx.send(()).unwrap();
            future::ok(()).boxed()
        });
        let client = WebSocketRpcClient::new();
        client.connect(Rc::new(transport)).await.unwrap();

        spawn_local(async move {
            client.reconnect().await.unwrap();
        });

        await_with_timeout(Box::pin(test_rx), 500)
            .await
            .unwrap()
            .unwrap();
    }

    /// Tests that [`RpcClient`] will only subscibe to the [`RpcTransport`]
    /// [`State`] updates if reconnection already started.
    ///
    /// # Algorithm
    ///
    /// 1. Mock [`RpcTransport::get_state`] to return [`State::Connecting`].
    ///
    /// 2. Mock [`RpcTransport::on_close`] to return [`CloseMsg::Abnormal`].
    ///
    /// 3. Mock [`RpcTransport::on_state_change`] to throw [`State::Open`].
    ///
    /// 4. Call [`RpcClient::reconnect`] and check that
    ///    [`RpcTransport::reconnect`] wasn't called.
    #[wasm_bindgen_test]
    async fn connecting_transport_state() {
        let mut transport = MockRpcTransport::new();
        transport
            .expect_get_state()
            .return_once(|| State::Connecting);
        transport
            .expect_on_message()
            .times(2)
            .returning(|| Ok(stream::pending().boxed()));
        transport
            .expect_set_close_reason()
            .times(1)
            .returning(|_| ());
        transport.expect_on_close().return_once(|| {
            Ok(stream::once(async { CloseMsg::Abnormal(1500) }).boxed())
        });
        let (on_state_change_tx, on_state_change_rx) = mpsc::unbounded();
        transport
            .expect_on_state_change()
            .return_once(|| on_state_change_rx.boxed());
        let client = WebSocketRpcClient::new();
        client.connect(Rc::new(transport)).await.unwrap();

        on_state_change_tx.unbounded_send(State::Open);
        client.reconnect().await.unwrap();
    }

    /// Tests that on [`RpcTransport`] [`State::Open`] [`RpcClient::reconnect`]
    /// will immediately resolved and [`RpcTransport::reconnect`] not
    /// called.
    ///
    /// # Algorithm
    ///
    /// 1. Mock [`RpcTransport::get_state`] to return [`State::Open`].
    ///
    /// 2. Call [`RpcClient::reconnect`] and check that
    ///    [`RpcTransport::reconnect`] function wasn't called.
    #[wasm_bindgen_test]
    async fn open_transport_state() {
        let mut transport = MockRpcTransport::new();
        transport.expect_get_state().return_once(|| State::Open);
        transport
            .expect_on_message()
            .times(2)
            .returning(|| Ok(stream::pending().boxed()));
        transport.expect_set_close_reason().return_once(|_| ());
        transport.expect_on_close().return_once(|| {
            Ok(stream::once(async { CloseMsg::Abnormal(1500) }).boxed())
        });
        let client = WebSocketRpcClient::new();
        client.connect(Rc::new(transport)).await.unwrap();
        client.reconnect().await.unwrap();
    }
}

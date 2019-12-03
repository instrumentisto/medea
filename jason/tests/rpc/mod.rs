//! Tests for [`medea_jason::rpc::RpcClient`].

mod websocket;

use std::{
    collections::HashMap,
    rc::Rc,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
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
    ClientDisconnect, CloseMsg, MockRpcTransport, RpcClient, WebSocketRpcClient,
};
use wasm_bindgen_futures::spawn_local;
use wasm_bindgen_test::*;

use crate::resolve_after;

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
    let srv_event = Event::PeersRemoved { peer_ids: vec![] };
    let srv_event_cloned = srv_event.clone();

    let mut transport = MockRpcTransport::new();
    transport.expect_on_message().return_once(move || {
        Ok(
            stream::once(async move { Ok(ServerMsg::Event(srv_event_cloned)) })
                .boxed(),
        )
    });
    transport.expect_send().return_once(|_| Ok(()));
    transport
        .expect_on_close()
        .return_once(|| Ok(future::pending().boxed()));
    transport.expect_set_close_reason().return_const(());

    let ws = WebSocketRpcClient::new(10);

    let mut stream = ws.subscribe();
    ws.connect(Rc::new(transport)).await.unwrap();
    assert_eq!(stream.next().await.unwrap(), srv_event);
}

/// Tests that [`WebSocketRpcClient`] sends [`Event::Ping`] to a server.
///
/// # Algorithm
///
/// 1. Connect [`WebSocketRpcClient`] with [`MockRpcTransport`].
///
/// 2. Subscribe to [`ClientMsg`]s which [`WebSocketRpcClient`] will send.
///
/// 3. Wait `600ms` for [`ClientMsg::Ping`].
#[wasm_bindgen_test]
async fn heartbeat() {
    let mut transport = MockRpcTransport::new();
    transport
        .expect_on_message()
        .return_once(move || Ok(stream::once(future::pending()).boxed()));
    transport
        .expect_on_close()
        .return_once(move || Ok(future::pending().boxed()));
    transport.expect_set_close_reason().return_const(());

    let counter = Arc::new(AtomicU64::new(1));
    let counter_clone = counter.clone();
    transport
        .expect_send()
        .times(3)
        .withf(move |msg: &ClientMsg| {
            if let ClientMsg::Ping(id) = msg {
                assert_eq!(*id, counter.fetch_add(1, Ordering::Relaxed));
            };
            true
        })
        .returning(|_| Ok(()));

    let ws = WebSocketRpcClient::new(50);
    ws.connect(Rc::new(transport)).await.unwrap();

    resolve_after(120).await.unwrap();
    assert!(counter_clone.load(Ordering::Relaxed) > 2);
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
    let ws = WebSocketRpcClient::new(500);
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

    match future::select(Box::pin(test_rx), Box::pin(resolve_after(1000))).await
    {
        Either::Left(_) => (),
        Either::Right(_) => panic!(
            "'unsub_drops_sub' lasts more that 1s. Most likely 'unsub' is \
             broken."
        ),
    }
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
        .return_once(move || Ok(stream::once(future::pending()).boxed()));
    transport
        .expect_on_close()
        .return_once(move || Ok(future::pending().boxed()));
    transport.expect_send().return_once(|_| Ok(()));
    transport.expect_set_close_reason().return_const(());
    let rpc_transport = Rc::new(transport);

    let ws = WebSocketRpcClient::new(500);
    ws.connect(rpc_transport.clone()).await.unwrap();
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
        .return_once(move || Ok(stream::once(future::pending()).boxed()));
    transport
        .expect_on_close()
        .return_once(move || Ok(future::pending().boxed()));
    transport.expect_send().returning(move |e| {
        on_send_tx.unbounded_send(e.clone()).unwrap();
        Ok(())
    });
    transport.expect_set_close_reason().return_const(());

    let ws = WebSocketRpcClient::new(500);
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

    match future::select(Box::pin(test_rx), Box::pin(resolve_after(1000))).await
    {
        Either::Left(_) => (),
        Either::Right(_) => {
            panic!("Command doesn't reach 'RpcTransport' within a 1s.")
        }
    }
}

mod on_close {
    //! Tests for [`WebSocketRpcClient::on_close`].

    use super::*;

    /// Returns [`WebSocketRpcClient`] which will be resolved
    /// [`WebSocketRpcClient::on_close`] [`Future`] with provided
    /// [`CloseMsg`].
    async fn get_client(close_msg: CloseMsg) -> WebSocketRpcClient {
        let mut transport = MockRpcTransport::new();
        transport
            .expect_on_message()
            .return_once(move || Ok(stream::once(future::pending()).boxed()));
        transport.expect_send().return_once(|_| Ok(()));
        transport
            .expect_on_close()
            .return_once(move || Ok(Box::pin(async { Ok(close_msg) })));

        let ws = WebSocketRpcClient::new(500);
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

mod transport_close_reason_on_drop {
    //! Tests which checks that when [`WebSocketRpcClient`] is dropped the right
    //! close reason is provided to [`RpcTransport`].

    use super::*;

    /// Returns [`WebSocketRpcClient`] and [`oneshot::Receiver`] which will be
    /// resolved with [`RpcTransport`]'s close reason
    /// ([`ClientDisconnect`]).
    async fn get_client(
    ) -> (WebSocketRpcClient, oneshot::Receiver<ClientDisconnect>) {
        let mut transport = MockRpcTransport::new();
        transport
            .expect_on_message()
            .return_once(move || Ok(stream::once(future::pending()).boxed()));
        transport.expect_send().return_once(|_| Ok(()));
        transport
            .expect_on_close()
            .return_once(|| Ok(future::pending().boxed()));
        let (test_tx, test_rx) = oneshot::channel();
        transport
            .expect_set_close_reason()
            .return_once(move |reason| {
                test_tx.send(reason).unwrap();
            });

        let ws = WebSocketRpcClient::new(500);
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

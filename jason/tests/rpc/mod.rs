//! Tests for [`medea_jason::rpc::RpcClient`].

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
use medea_client_api_proto::{ClientMsg, Command, Event, PeerId, ServerMsg};
use medea_jason::rpc::{MockRpcTransport, RpcClient, WebSocketRpcClient};
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

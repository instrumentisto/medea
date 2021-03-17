//! Tests for [`medea_jason::rpc::Heartbeat`].

use std::{rc::Rc, time::Duration};

use futures::{
    channel::{mpsc, oneshot},
    stream, StreamExt,
};
use medea_client_api_proto::{ClientMsg, ServerMsg};
use medea_jason::rpc::{
    websocket::MockRpcTransport, Heartbeat, IdleTimeout, PingInterval,
    RpcTransport,
};
use wasm_bindgen_test::*;

use crate::{delay_for, timeout};

wasm_bindgen_test_configure!(run_in_browser);

/// Tests that [`ClientMsg::Pong`] will be sent after received
/// [`ServerMsg::Ping`].
///
/// # Algorithm
///
/// 1. Mock [`RpcClient::on_message`] and send to this [`Stream`]
///    [`ServerMsg::Ping`].
///
/// 2. Mock [`RpcClient::send`] and check that [`ClientMsg::Pong`] was sent.
#[wasm_bindgen_test]
async fn sends_pong_on_received_ping() {
    let mut transport = MockRpcTransport::new();
    let (on_message_tx, on_message_rx) = mpsc::unbounded();
    transport
        .expect_on_message()
        .return_once(|| Box::pin(on_message_rx));
    let (test_tx, test_rx) = oneshot::channel();
    transport.expect_send().return_once(move |msg| {
        test_tx.send(msg.clone()).unwrap();
        Ok(())
    });

    let _hb = Heartbeat::start(
        Rc::new(transport),
        PingInterval(Duration::from_secs(10).into()),
        IdleTimeout(Duration::from_secs(10).into()),
    );

    on_message_tx.unbounded_send(ServerMsg::Ping(2)).unwrap();
    timeout(100, async move {
        let msg = test_rx.await.unwrap();
        match msg {
            ClientMsg::Pong(_) => (),
            _ => panic!("Received not pong message! Message: {:?}", msg),
        }
    })
    .await
    .unwrap();
}

/// Tests that idle timeout works.
///
/// # Algorithm
///
/// 1. Mock [`RpcTransport::on_message`] to return infinite [`Stream`].
///
/// 2. Wait for [`Heartbeat::on_idle`] resolving.
#[wasm_bindgen_test]
async fn on_idle_works() {
    let mut transport = MockRpcTransport::new();
    transport
        .expect_on_message()
        .return_once(|| stream::pending().boxed());
    transport.expect_send().return_once(|_| Ok(()));

    let hb = Heartbeat::start(
        Rc::new(transport),
        PingInterval(Duration::from_millis(50).into()),
        IdleTimeout(Duration::from_millis(100).into()),
    );

    timeout(120, hb.on_idle().next()).await.unwrap().unwrap();
}

/// Tests that [`Heartbeat`] will try send [`ClientMsg::Pong`] if
/// no [`ServerMsg::Ping`]s received within `ping_interval * 2`.
///
/// # Algorithm
///
/// 1. Create [`Heartbeat`] with 10 milliseconds `ping_interval`.
///
/// 2. Mock [`RpcTransport::on_message`] to return infinite [`Stream`].
///
/// 3. Mock [`RpcTransport::send`] and wait for [`ClientMsg::Pong`] (with 25
///    milliseconds timeout).
#[wasm_bindgen_test]
async fn pre_sends_pong() {
    let mut transport = MockRpcTransport::new();
    transport
        .expect_on_message()
        .return_once(|| stream::pending().boxed());
    let (on_message_tx, mut on_message_rx) = mpsc::unbounded();
    transport.expect_send().return_once(move |msg| {
        on_message_tx.unbounded_send(msg.clone()).unwrap();
        Ok(())
    });

    let _hb = Heartbeat::start(
        Rc::new(transport),
        PingInterval(Duration::from_millis(10).into()),
        IdleTimeout(Duration::from_millis(100).into()),
    );

    let msg = timeout(25, on_message_rx.next()).await.unwrap().unwrap();
    match msg {
        ClientMsg::Pong(n) => {
            assert_eq!(n, 1);
        }
        _ => {
            panic!("Received not pong message! Message: {:?}", msg);
        }
    }
}

/// Tests that [`RpcTransport`] will be dropped when [`Heartbeat`] was
/// dropped.
#[wasm_bindgen_test]
async fn transport_is_dropped_when_hearbeater_is_dropped() {
    let mut transport = MockRpcTransport::new();
    transport
        .expect_on_message()
        .returning(|| stream::pending().boxed());
    let transport: Rc<dyn RpcTransport> = Rc::new(transport);

    let hb = Heartbeat::start(
        Rc::clone(&transport),
        PingInterval(Duration::from_secs(3).into()),
        IdleTimeout(Duration::from_secs(10).into()),
    );
    assert!(Rc::strong_count(&transport) > 1);
    drop(hb);
    delay_for(100).await;
    assert_eq!(Rc::strong_count(&transport), 1);
}

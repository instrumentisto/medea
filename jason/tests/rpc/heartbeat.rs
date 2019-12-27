//! Tests for [`medea_jason::rpc::Heartbeat`].

use std::{rc::Rc, time::Duration};

use futures::{
    channel::{mpsc, oneshot},
    stream, FutureExt, StreamExt,
};
use medea_client_api_proto::{ClientMsg, ServerMsg};
use medea_jason::rpc::{
    Heartbeat, IdleTimeout, MockRpcTransport, PingInterval,
};
use wasm_bindgen_test::*;

use crate::await_with_timeout;

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
    let hb = Heartbeat::new();
    let mut transport = MockRpcTransport::new();
    let (on_message_tx, on_message_rx) = mpsc::unbounded();
    transport
        .expect_on_message()
        .return_once(|| Ok(Box::pin(on_message_rx)));
    let (test_tx, test_rx) = oneshot::channel();
    transport.expect_send().return_once(move |msg| {
        test_tx.send(msg.clone()).unwrap();
        Ok(())
    });
    hb.start(
        IdleTimeout(Duration::from_secs(3).into()),
        PingInterval(Duration::from_secs(3).into()),
        Rc::new(transport),
    )
    .unwrap();
    on_message_tx.unbounded_send(ServerMsg::Ping(2)).unwrap();
    await_with_timeout(
        Box::pin(async move {
            match test_rx.await.unwrap() {
                ClientMsg::Pong(_) => (),
                ClientMsg::Command(cmd) => {
                    panic!("Received not pong message! Command: {:?}", cmd)
                }
            }
        }),
        100,
    )
    .await
    .unwrap();
}

/// Tests that IDLE timeout works.
///
/// # Algorithm
///
/// 1. Mock [`RpcTransport::on_message`] to return infinite [`Stream`].
///
/// 2. Wait for [`Heartbeat::on_idle`] resolving.
#[wasm_bindgen_test]
async fn on_idle_works() {
    let hb = Heartbeat::new();
    let mut transport = MockRpcTransport::new();
    transport
        .expect_on_message()
        .return_once(|| Ok(stream::pending().boxed()));
    transport.expect_send().return_once(|_| Ok(()));

    let mut on_idle_stream = hb.on_idle();
    hb.start(
        IdleTimeout(Duration::from_millis(100).into()),
        PingInterval(Duration::from_millis(50).into()),
        Rc::new(transport),
    )
    .unwrap();

    await_with_timeout(Box::pin(on_idle_stream.next()), 110)
        .await
        .unwrap()
        .unwrap();
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
    let hb = Heartbeat::new();
    let mut transport = MockRpcTransport::new();
    transport
        .expect_on_message()
        .return_once(|| Ok(stream::pending().boxed()));
    let (on_message_tx, mut on_message_rx) = mpsc::unbounded();
    transport.expect_send().return_once(move |msg| {
        on_message_tx.unbounded_send(msg.clone()).unwrap();
        Ok(())
    });

    hb.start(
        IdleTimeout(Duration::from_millis(100).into()),
        PingInterval(Duration::from_millis(10).into()),
        Rc::new(transport),
    )
    .unwrap();

    match await_with_timeout(on_message_rx.next().boxed(), 25)
        .await
        .unwrap()
        .unwrap()
    {
        ClientMsg::Pong(n) => {
            assert_eq!(n, 1);
        }
        ClientMsg::Command(cmd) => {
            panic!("Received not pong message! Command: {:?}", cmd);
        }
    }
}

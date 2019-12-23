use std::{rc::Rc, time::Duration};

use futures::{
    channel::{mpsc, oneshot},
    future, stream, FutureExt, StreamExt,
};
use medea_client_api_proto::{ClientMsg, ServerMsg};
use medea_jason::rpc::{
    Heartbeat, HeartbeatError, IdleTimeout, MockRpcTransport, PingInterval,
};
use wasm_bindgen_test::*;

use crate::{await_with_timeout, resolve_after};

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn sends_pong_on_received_ping() {
    let hb = Heartbeat::new(
        IdleTimeout(Duration::from_secs(3).into()),
        PingInterval(Duration::from_secs(3).into()),
    );
    let mut transport = MockRpcTransport::new();
    let (on_message_tx, on_message_rx) = mpsc::unbounded();
    transport
        .expect_on_message()
        .return_once(|| Ok(Box::pin(on_message_rx)));
    let (test_tx, test_rx) = oneshot::channel();
    transport.expect_send().return_once(move |msg| {
        test_tx.send(msg.clone());
        Ok(())
    });
    hb.start(Rc::new(transport));
    on_message_tx.unbounded_send(ServerMsg::Ping(2));
    await_with_timeout(
        Box::pin(async move {
            match test_rx.await.unwrap() {
                ClientMsg::Pong(_) => (),
                _ => panic!("aksjdfklajds"),
            }
        }),
        100,
    )
    .await
    .unwrap();
}

#[wasm_bindgen_test]
async fn on_idle_works() {
    let hb = Heartbeat::new(
        IdleTimeout(Duration::from_millis(100).into()),
        PingInterval(Duration::from_millis(50).into()),
    );
    let mut transport = MockRpcTransport::new();
    transport
        .expect_on_message()
        .return_once(|| Ok(stream::pending().boxed()));
    transport.expect_send().return_once(|_| Ok(()));

    let mut on_idle_stream = hb.on_idle();
    hb.start(Rc::new(transport));

    await_with_timeout(Box::pin(on_idle_stream.next()), 110)
        .await
        .unwrap()
        .unwrap();
}

#[wasm_bindgen_test]
async fn pre_sends_pong() {
    let hb = Heartbeat::new(
        IdleTimeout(Duration::from_millis(100).into()),
        PingInterval(Duration::from_millis(10).into()),
    );
    let mut transport = MockRpcTransport::new();
    transport
        .expect_on_message()
        .return_once(|| Ok(stream::pending().boxed()));
    let (on_message_tx, mut on_message_rx) = mpsc::unbounded();
    transport.expect_send().return_once(move |msg| {
        on_message_tx.unbounded_send(msg.clone());
        Ok(())
    });

    hb.start(Rc::new(transport));

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

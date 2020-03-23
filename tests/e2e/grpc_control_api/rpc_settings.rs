//! Tests for the RPC settings in `Member` element spec.

use std::time::{Duration, Instant};

use futures::channel::oneshot;

use crate::{
    grpc_control_api::{ControlClient, MemberBuilder, RoomBuilder},
    signalling::{ConnectionEvent, TestMember},
};

/// Tests that RPC settings in `Member` element spec works.
///
/// # Algorithm
///
/// 1. Create `Room` with `Member` with `ping_interval: 10`, `idle_timeout: 2`,
///    `reconnect_timeout: 0`;
///
/// 2. Connect with [`TestMember`] as created `Member`;
///
/// 3. When connection will be started, store [`Instant`];
///
/// 4. When connection will be stopped verify that less than three seconds
///    elapse between the start and end of the session.
#[actix_rt::test]
async fn rpc_settings_from_spec_works() {
    const ROOM_ID: &str = "rpc_settings_from_spec_works";

    let mut control_client = ControlClient::new().await;
    let create_room = RoomBuilder::default()
        .id(ROOM_ID)
        .add_member(
            MemberBuilder::default()
                .id("member")
                .credentials("test")
                .ping_interval(10u64)
                .idle_timeout(2u64)
                .reconnect_timeout(0u64)
                .build()
                .unwrap(),
        )
        .build()
        .unwrap()
        .build_request(String::new());
    control_client.create(create_room).await;

    let (test_end_tx, test_end_rx) = oneshot::channel();
    let mut test_end_tx = Some(test_end_tx);

    let mut connection_start_time = None;
    TestMember::start(
        format!("ws://127.0.0.1:8080/ws/{}/member/test", ROOM_ID),
        Box::new(|_, _, _| {}),
        Box::new(move |event| match event {
            ConnectionEvent::Started => {
                connection_start_time = Some(Instant::now());
            }
            ConnectionEvent::Stopped => {
                let deadline = connection_start_time.unwrap()
                    + Duration::from_millis(2500);
                if deadline < Instant::now() {
                    unreachable!()
                }

                if let Some(test_end_tx) = test_end_tx.take() {
                    test_end_tx.send(()).unwrap();
                }
            }
        }),
        Some(Duration::from_secs(5)),
    );

    test_end_rx.await.unwrap();
}

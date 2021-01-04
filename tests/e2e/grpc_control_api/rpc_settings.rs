//! Tests for the RPC settings in `Member` element spec.

use std::time::{Duration, Instant};

use futures::channel::oneshot;
use medea_control_api_proto::grpc::api::member::Credentials;

use crate::{
    grpc_control_api::{ControlClient, MemberBuilder, RoomBuilder},
    signalling::{ConnectionEvent, TestMember},
};

/// Tests that RPC settings in `Member` element spec works.
///
/// # Algorithm
///
/// 1. Create `Room` with `Member` with `ping_interval: 10`, `idle_timeout: 3`,
///    `reconnect_timeout: 0`;
///
/// 2. Connect with [`TestMember`] as created `Member`;
///
/// 3. When connection will be started, store [`Instant`];
///
/// 4. Wait for connection drop because idle, and verify that diff between
/// connection open and drop if >3 and <5.
#[actix_rt::test]
async fn rpc_settings_from_spec_works() {
    const ROOM_ID: &str = "rpc_settings_from_spec_works";

    let mut control_client = ControlClient::new().await;
    let create_room = RoomBuilder::default()
        .id(ROOM_ID)
        .add_member(
            MemberBuilder::default()
                .id("member")
                .credentials(Credentials::Plain(String::from("test")))
                .ping_interval(Some(Duration::from_secs(10)))
                .idle_timeout(Some(Duration::from_secs(1)))
                .reconnect_timeout(Some(Duration::from_secs(0)))
                .build()
                .unwrap(),
        )
        .build()
        .unwrap()
        .build_request(String::new());
    control_client.create(create_room).await;

    let (test_end_tx, test_end_rx) = oneshot::channel();
    let mut test_end_tx = Some(test_end_tx);

    let mut opened = None;
    TestMember::start(
        format!("ws://127.0.0.1:8080/ws/{}/member?token=test", ROOM_ID),
        None,
        Some(Box::new(move |event| match event {
            ConnectionEvent::Started => {
                opened = Some(Instant::now());
            }
            ConnectionEvent::Stopped => {
                let diff = Instant::now() - opened.unwrap();

                assert!(diff > Duration::from_secs(1));
                assert!(diff < Duration::from_secs(3));

                test_end_tx.take().unwrap().send(()).unwrap();
            }
            _ => {}
        })),
        Some(Duration::from_secs(10)),
    );

    test_end_rx.await.unwrap();
}

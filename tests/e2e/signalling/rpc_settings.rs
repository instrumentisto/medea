//! Tests for the RPC settings in `Member` element spec.

use std::time::Duration;

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
/// connection open and drop if >3 and <4.
#[actix_rt::test]
async fn rpc_settings_server_msg() {
    const ROOM_ID: &str = "rpc_settings_server_msg";

    let mut control_client = ControlClient::new().await;
    let create_room = RoomBuilder::default()
        .id(ROOM_ID)
        .add_member(
            MemberBuilder::default()
                .id("member")
                .credentials("test")
                .ping_interval(Some(Duration::from_secs(111)))
                .idle_timeout(Some(Duration::from_secs(222)))
                .reconnect_timeout(Some(Duration::from_secs(0)))
                .build()
                .unwrap(),
        )
        .build()
        .unwrap()
        .build_request(String::new());
    control_client.create(create_room).await;

    TestMember::start(
        format!("ws://127.0.0.1:8080/ws/{}/member/test", ROOM_ID),
        None,
        Some(Box::new(|event| {
            if let ConnectionEvent::SettingsReceived(settings) = event {
                assert_eq!(settings.idle_timeout_ms, 222_000);
                assert_eq!(settings.ping_interval_ms, 111_000);
            }
        })),
        Some(Duration::from_secs(10)),
    );
}

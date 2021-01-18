//! Tests for the RPC settings in `Member` element spec.

use std::time::Duration;

use futures::channel::oneshot;
use medea_control_api_proto::grpc::api::member::Credentials;

use crate::{
    grpc_control_api::{ControlClient, MemberBuilder, RoomBuilder},
    signalling::{ConnectionEvent, TestMember},
};

/// Tests that RPC settings configured via Control API request are propagated in
/// [`ServerMsg::RpcSettings`] server message.
#[actix_rt::test]
async fn rpc_settings_server_msg() {
    const ROOM_ID: &str = "rpc_settings_server_msg";
    const PING_INTERVAL_SECS: u32 = 1;
    const IDLE_TIMEOUT_SECS: u32 = 1;

    let mut control_client = ControlClient::new().await;
    let create_room = RoomBuilder::default()
        .id(ROOM_ID)
        .add_member(
            MemberBuilder::default()
                .id("member")
                .credentials(Credentials::Plain(String::from("test")))
                .ping_interval(Some(Duration::from_secs(
                    PING_INTERVAL_SECS.into(),
                )))
                .idle_timeout(Some(Duration::from_secs(
                    IDLE_TIMEOUT_SECS.into(),
                )))
                .reconnect_timeout(Some(Duration::from_secs(0)))
                .build()
                .unwrap(),
        )
        .build()
        .unwrap()
        .build_request(String::new());
    control_client.create(create_room).await;

    let (end_tx, end_rx) = oneshot::channel();
    let mut end_tx = Some(end_tx);
    let mut is_initial_settings_received = false;
    TestMember::start(
        format!("ws://127.0.0.1:8080/ws/{}/member?token=test", ROOM_ID),
        None,
        Some(Box::new(move |event| {
            if let ConnectionEvent::SettingsReceived(settings) = event {
                if is_initial_settings_received {
                    assert_eq!(
                        settings.idle_timeout_ms,
                        IDLE_TIMEOUT_SECS * 1000
                    );
                    assert_eq!(
                        settings.ping_interval_ms,
                        PING_INTERVAL_SECS * 1000
                    );
                    end_tx.take().unwrap().send(()).unwrap();
                } else {
                    is_initial_settings_received = true;
                }
            }
        })),
        Some(Duration::from_secs(10)),
    );

    end_rx.await.unwrap();
}

//! E2E tests for `Member` Control API callbacks.

use std::time::Duration;

use actix::{clock::sleep, Addr, Context};
use actix_http::ws::CloseCode;
use function_name::named;
use medea_client_api_proto::Event as RpcEvent;
use medea_control_api_proto::grpc::callback as proto;
use proto::request::Event;

use crate::{
    callbacks::{GetCallbacks, GrpcCallbackServer},
    grpc_control_api::{ControlClient, MemberBuilder, RoomBuilder},
    signalling::{CloseSocket, TestMember},
    test_name,
};

/// Type for [`Future`] item in `callback_test` function.
type CallbackTestItem = (Addr<TestMember>, Addr<GrpcCallbackServer>);

/// Creates `Room` with this spec:
///
/// ```yaml
/// kind: Room
/// id: {{ PROVIDED NAME }}
/// spec:
///    pipeline:
///      {{ PROVIDED NAME }}:
///        kind: Member
///        on_join: "grpc://127.0.0.1:{{ PROVIDED PORT }}"
///        on_leave: "grpc://127.0.0.1:{{ PROVIDED PORT }}"
/// ```
///
/// Then, returns [`Future`] which resolves with [`TestMember`]
/// connected to created `Room` and [`GrpcCallbackServer`] which
/// will receive all callbacks from Medea.
async fn callback_test(name: &str, port: u16) -> CallbackTestItem {
    let callback_server = super::run(port);
    let mut control_client = ControlClient::new().await;
    let member = RoomBuilder::default()
        .id(name)
        .add_member(
            MemberBuilder::default()
                .id(String::from(name))
                .on_leave(format!("grpc://127.0.0.1:{}", port))
                .on_join(format!("grpc://127.0.0.1:{}", port))
                .build()
                .unwrap(),
        )
        .build()
        .unwrap()
        .build_request(String::new());
    let create_response = control_client.create(member).await;

    let on_event =
        move |_: &RpcEvent, _: &mut Context<TestMember>, _: Vec<&RpcEvent>| {};
    let client = TestMember::connect(
        create_response.get(name).unwrap(),
        Some(Box::new(on_event)),
        None,
        TestMember::DEFAULT_DEADLINE,
        true,
        true,
    )
    .await;
    (client, callback_server)
}

/// Checks that `on_join` callback works.
///
/// # Algorithm
///
/// 1. Start test callback server and connect [`TestMember`] to it.
///
/// 2. Wait `500ms`.
///
/// 3. Check that test callback server receives `on_join` callback.
#[actix_rt::test]
#[named]
async fn on_join() {
    let (_, callback_server) = callback_test(test_name!(), 9096).await;
    sleep(Duration::from_millis(500)).await;
    let callbacks = callback_server.send(GetCallbacks).await.unwrap().unwrap();
    let on_joins_count = callbacks
        .into_iter()
        .filter(|r| {
            if let Some(Event::OnJoin(_)) = &r.event {
                true
            } else {
                false
            }
        })
        .count();
    assert_eq!(on_joins_count, 1);
}

/// Checks that `on_leave` callback works on normal client disconnect.
///
/// # Algorithm
///
/// 1. Start test callback server and connect [`TestMember`] to it.
///
/// 2. Close [`TestMember`]'s socket with [`CloseCode::Normal`].
///
/// 3. Wait `500ms`.
///
/// 4. Check that test callback server receives `on_leave` callback with
/// [`proto::on_leave::Reason::DISONNECTED`].
#[actix_rt::test]
#[named]
async fn on_leave_normally_disconnected() {
    let (client, callback_server) = callback_test(test_name!(), 9097).await;
    client.send(CloseSocket(CloseCode::Normal)).await.unwrap();
    sleep(Duration::from_millis(500)).await;

    let callbacks = callback_server.send(GetCallbacks).await.unwrap().unwrap();

    let on_leaves_count = callbacks
        .into_iter()
        .filter_map(|req| {
            if let Some(Event::OnLeave(on_leave)) = req.event {
                Some(on_leave.reason)
            } else {
                None
            }
        })
        .filter(|reason| {
            reason == &(proto::on_leave::Reason::Disconnected as i32)
        })
        .count();
    assert_eq!(on_leaves_count, 1);
}

/// Checks that `on_leave` callback works when connection with client was lost.
///
/// # Algorithm
///
/// 1. Start test callback server and connect [`TestMember`] to it.
///
/// 2. Close [`TestMember`]'s socket with [`CloseCode::Abnormal`].
///
/// 3. Wait `500ms`.
///
/// 4. Check that test callback server receives `on_leave` callback with
/// [`proto::on_leave::Reason::LOST_CONNECTION`].
#[actix_rt::test]
#[named]
async fn on_leave_on_connection_loss() {
    let (client, callback_server) = callback_test(test_name!(), 9098).await;

    client.send(CloseSocket(CloseCode::Abnormal)).await.unwrap();
    sleep(Duration::from_millis(500)).await;

    let callbacks = callback_server.send(GetCallbacks).await.unwrap().unwrap();

    let on_leaves_count = callbacks
        .into_iter()
        .filter_map(|req| {
            if let Some(Event::OnLeave(on_leave)) = req.event {
                Some(on_leave.reason)
            } else {
                None
            }
        })
        .filter(|reason| {
            reason == &(proto::on_leave::Reason::LostConnection as i32)
        })
        .count();
    assert_eq!(on_leaves_count, 1);
}

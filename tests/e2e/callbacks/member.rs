//! E2E tests for `Member` Control API callbacks.

use std::time::Duration;

use actix::{clock::delay_for, Addr, Context};
use actix_http::ws::CloseCode;
use medea_client_api_proto::Event;
use medea_control_api_proto::grpc::callback::{
    OnLeave_Reason as OnLeaveReason, Request,
};

use crate::{
    callbacks::{GetCallbacks, GrpcCallbackServer},
    grpc_control_api::{ControlClient, MemberBuilder, RoomBuilder},
    signalling::{CloseSocket, TestMember},
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
async fn callback_test(name: &'static str, port: u16) -> CallbackTestItem {
    let callback_server = super::run(port);
    let control_client = ControlClient::new();
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
    let create_response = control_client.create(&member);

    let on_event =
        move |_: &Event, _: &mut Context<TestMember>, _: Vec<&Event>| {};
    let deadline = Some(Duration::from_secs(5));
    let client = TestMember::connect(
        create_response.get(name).unwrap(),
        Box::new(on_event),
        deadline,
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
/// 2. Wait `50ms`.
///
/// 3. Check that test callback server receives `on_join` callback.
#[actix_rt::test]
async fn on_join() {
    const TEST_NAME: &str = "member_callback_on_join";

    let (_, callback_server) = callback_test(TEST_NAME, 9099).await;
    delay_for(Duration::from_millis(300)).await;
    let callbacks = callback_server.send(GetCallbacks).await.unwrap().unwrap();
    let on_joins_count =
        callbacks.into_iter().filter(Request::has_on_join).count();
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
/// 3. Wait `50ms`.
///
/// 4. Check that test callback server receives `on_leave` callback with
/// [`OnLeaveReason::DISONNECTED`].
#[actix_rt::test]
async fn on_leave_normally_disconnected() {
    const TEST_NAME: &str = "member_callback_on_leave";

    let (client, callback_server) = callback_test(TEST_NAME, 9098).await;
    client.send(CloseSocket(CloseCode::Normal)).await.unwrap();
    delay_for(Duration::from_millis(300)).await;

    let callbacks = callback_server.send(GetCallbacks).await.unwrap().unwrap();

    let on_leaves_count = callbacks
        .into_iter()
        .filter_map(|mut req| {
            if req.has_on_leave() {
                Some(req.take_on_leave().reason)
            } else {
                None
            }
        })
        .filter(|reason| reason == &OnLeaveReason::DISCONNECTED)
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
/// 3. Wait `50ms`.
///
/// 4. Check that test callback server receives `on_leave` callback with
/// [`OnLeaveReason::LOST_CONNECTION`].
#[actix_rt::test]
async fn on_leave_on_connection_loss() {
    const TEST_NAME: &str = "member_callback_on_leave_on_connection_loss";

    let (client, callback_server) = callback_test(TEST_NAME, 9096).await;

    client.send(CloseSocket(CloseCode::Abnormal)).await.unwrap();
    delay_for(Duration::from_millis(1100)).await;

    let callbacks = callback_server.send(GetCallbacks).await.unwrap().unwrap();

    let on_leaves_count = callbacks
        .into_iter()
        .filter_map(|mut req| {
            if req.has_on_leave() {
                Some(req.take_on_leave().reason)
            } else {
                None
            }
        })
        .filter(|reason| reason == &OnLeaveReason::LOST_CONNECTION)
        .count();
    assert_eq!(on_leaves_count, 1);
}

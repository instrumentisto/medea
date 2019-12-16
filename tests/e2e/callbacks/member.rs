//! E2E tests for `Member` Control API callbacks.

use std::time::Duration;

use actix::{Addr, Arbiter, Context, System};
use actix_http::ws::CloseCode;
use futures::Future;
use medea_client_api_proto::Event;
use medea_control_api_proto::grpc::callback::OnLeave_Reason as OnLeaveReason;

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
///      test-member:
///        kind: Member
///        on_join: "grpc://127.0.0.1:{{ PROVIDED PORT }}"
///        on_leave: "grpc://127.0.0.1:{{ PROVIDED PORT }}"
/// ```
///
/// Then, returns [`Future`] which resolves with [`TestMember`]
/// connected to created `Room` and [`GrpcCallbackServer`] which
/// will receive all callbacks from Medea.
fn callback_test(
    name: &str,
    port: u16,
) -> impl Future<Item = CallbackTestItem, Error = ()> {
    let callback_server = super::run(port);
    let control_client = ControlClient::new();
    let member = RoomBuilder::default()
        .id(name)
        .add_member(
            MemberBuilder::default()
                .id("test-member")
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
    TestMember::connect(
        create_response.get("test-member").unwrap(),
        Box::new(on_event),
        deadline,
    )
    .map(move |client| (client, callback_server))
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
#[test]
fn on_join() {
    const TEST_NAME: &str = "member_callback_on_join";
    let sys = System::new(TEST_NAME);

    Arbiter::spawn(
        callback_test(TEST_NAME, 9099)
            .and_then(move |(_, callback_server)| {
                std::thread::sleep(Duration::from_millis(50));
                callback_server.send(GetCallbacks).map_err(|_| ())
            })
            .map(|callbacks_result| {
                let on_joins_count = callbacks_result
                    .unwrap()
                    .into_iter()
                    .filter(|req| req.has_on_join())
                    .count();
                assert_eq!(on_joins_count, 1);
                System::current().stop();
            }),
    );

    sys.run().unwrap()
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
#[test]
fn on_leave_normally_disconnected() {
    const TEST_NAME: &str = "member_callback_on_leave";
    let sys = System::new(TEST_NAME);

    Arbiter::spawn(
        callback_test(TEST_NAME, 9098)
            .and_then(|(client, callback_server)| {
                client
                    .send(CloseSocket(CloseCode::Normal))
                    .map_err(|e| panic!("{:?}", e))
                    .map(move |_| callback_server)
            })
            .and_then(move |callback_server| {
                std::thread::sleep(Duration::from_millis(50));
                callback_server.send(GetCallbacks).map_err(|_| ())
            })
            .map(|callbacks_result| {
                let on_leaves_count = callbacks_result
                    .unwrap()
                    .into_iter()
                    .filter(|req| req.has_on_leave())
                    .map(|mut req| req.take_on_leave().reason)
                    .filter(|reason| reason == &OnLeaveReason::DISCONNECTED)
                    .count();
                assert_eq!(on_leaves_count, 1);
                System::current().stop();
            }),
    );

    sys.run().unwrap()
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
#[test]
fn on_leave_on_connection_loss() {
    const TEST_NAME: &str = "member_callback_on_leave_on_connection_loss";
    let sys = System::new(TEST_NAME);

    Arbiter::spawn(
        callback_test(TEST_NAME, 9096)
            .and_then(|(client, callback_server)| {
                client
                    .send(CloseSocket(CloseCode::Abnormal))
                    .map_err(|e| panic!("{:?}", e))
                    .map(move |_| callback_server)
            })
            .and_then(move |callback_server| {
                // Wait for 'idle_timeout'.
                std::thread::sleep(Duration::from_millis(1100));
                callback_server.send(GetCallbacks).map_err(|_| ())
            })
            .map(|callbacks_result| {
                let on_leaves_count = callbacks_result
                    .unwrap()
                    .into_iter()
                    .filter(|req| req.has_on_leave())
                    .map(|mut req| req.take_on_leave().reason)
                    .filter(|reason| reason == &OnLeaveReason::LOST_CONNECTION)
                    .count();
                assert_eq!(on_leaves_count, 1);
                System::current().stop();
            }),
    );

    sys.run().unwrap()
}

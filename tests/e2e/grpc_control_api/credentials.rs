use actix::Context;
use function_name::named;
use futures::{channel::mpsc, StreamExt};
use medea_client_api_proto::{CloseReason, Event};
use medea_control_api_proto::grpc::api::member::Credentials;

use crate::{
    grpc_control_api::ControlClient, signalling::TestMember, test_name,
};

use super::{MemberBuilder, RoomBuilder};

/// [Argon2] hash of the word `medea`.
///
/// [Argon2]: https://en.wikipedia.org/wiki/Argon2
const MEDEA_CRED_HASH: &str =
    "$argon2i$v=19$m=16,t=2,p=1$ZHNtcEFmVnREZkRtNk9hOA$6z1z/KA2FnBJA7fqqpdBQA";

/// Creates new `Room` with a provided `name` and `credentials`.
///
/// ## Spec
///
/// ```yaml
/// kind: Room
/// id: {{ room_id }}
/// spec:
///   pipeline:
///     member:
///       kind: Member
///       credentials:
///         plain: {{ credentials }} # Credentials::Plain
///         hash: {{ credentials }} # Credentials::Hash
///       spec:
///         pipeline:
///           play:
///             kind: WebRtcPlayEndpoint
///             spec:
///               src: "local://{{ room_id }}/publisher/publish"
/// ```
async fn create_test_room(name: &str, credentials: Credentials) {
    let mut control_client = ControlClient::new().await;

    let create_room = RoomBuilder::default()
        .id(name)
        .add_member(
            MemberBuilder::default()
                .id("member")
                .credentials(credentials)
                .build()
                .unwrap(),
        )
        .build()
        .unwrap()
        .build_request("");

    control_client.create(create_room).await;
}

/// Joins `Room` with a provided URL.
///
/// Waits for first [`Event`] from server and returns it.
async fn join_room(url: &str) -> Option<Event> {
    let (tx, mut rx) = mpsc::unbounded();
    TestMember::connect(
        url,
        Some(Box::new(
            move |event: &Event,
                  _: &mut Context<TestMember>,
                  _: Vec<&Event>| {
                tx.unbounded_send(event.clone()).unwrap();
            },
        )),
        None,
        TestMember::DEFAULT_DEADLINE,
        false,
        false,
    )
    .await;

    rx.next().await
}

/// Checks that Client will be rejected on invalid plain text credentials.
#[actix_rt::test]
#[named]
async fn invalid_plain_credentials() {
    create_test_room(test_name!(), Credentials::Plain(String::from("test")))
        .await;

    assert_eq!(
        join_room(&format!(
            "ws://127.0.0.1:8080/ws/{}/member?token=test2",
            test_name!(),
        ))
        .await
        .unwrap(),
        Event::RoomLeft {
            close_reason: CloseReason::Rejected
        }
    );
}

/// Checks that Client will be rejected on invalid hash credentials.
#[actix_rt::test]
#[named]
async fn invalid_hash_credentials() {
    create_test_room(
        test_name!(),
        Credentials::Hash(MEDEA_CRED_HASH.to_string()),
    )
    .await;

    assert_eq!(
        join_room(&format!(
            "ws://127.0.0.1:8080/ws/{}/member?token=foobar",
            test_name!(),
        ))
        .await
        .unwrap(),
        Event::RoomLeft {
            close_reason: CloseReason::Rejected
        }
    );
}

/// Checks that Client will be accepted on valid plain text credentials.
#[actix_rt::test]
#[named]
async fn valid_hash_credentials() {
    create_test_room(
        test_name!(),
        Credentials::Hash(MEDEA_CRED_HASH.to_string()),
    )
    .await;

    assert_eq!(
        join_room(&format!(
            "ws://127.0.0.1:8080/ws/{}/member?token=medea",
            test_name!(),
        ))
        .await
        .unwrap(),
        Event::RoomJoined {
            member_id: "member".into(),
        }
    );
}

/// Checks that Client will be accepted on valid hash credentials.
#[actix_rt::test]
#[named]
async fn valid_plain_credentials() {
    create_test_room(test_name!(), Credentials::Plain("medea".to_string()))
        .await;

    assert_eq!(
        join_room(&format!(
            "ws://127.0.0.1:8080/ws/{}/member?token=medea",
            test_name!(),
        ))
        .await
        .unwrap(),
        Event::RoomJoined {
            member_id: "member".into(),
        }
    );
}

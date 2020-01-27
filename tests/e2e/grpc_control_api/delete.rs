//! Tests for `Delete` method of gRPC [Control API].
//!
//! The specificity of these tests is such that the `Get` method is also
//! being tested at the same time.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

use medea::api::control::error_codes::{
    ErrorCode as MedeaErrorCode, ErrorCode,
};

use super::{create_room_req, ControlClient};

/// Tests `Delete` method of [Medea]'s [Control API].
///
/// # Arguments
///
/// `room_id`: `Room` ID which will be created and from will be deleted
/// `Element`s,
///
/// `element_id`: `Element` ID which will be deleted from this `Room`,
///
/// `error_code`: [`ErrorCode`] which should be returned from [`ControlAPI`]
/// when we tries get deleted `Element`.
///
/// [Medea]: https://github.com/instrumentisto/medea
/// [Control API]: https://tinyurl.com/yxsqplq7
/// [`ErrorCode`]: medea::api::control::error_codes::ErrorCode
async fn test_for_delete(
    room_id: &str,
    element_id: &str,
    error_code: MedeaErrorCode,
) {
    let mut client = ControlClient::new().await;
    client.create(create_room_req(room_id)).await;

    client.try_get(element_id).await.unwrap();

    client.delete(&[element_id]).await.unwrap();

    let get_room_err = match client.try_get(element_id).await {
        Ok(_) => panic!("{} not deleted!", element_id),
        Err(e) => e,
    };
    assert_eq!(get_room_err.code, error_code as u32);
}

#[actix_rt::test]
async fn room() {
    const TEST_NAME: &str = "delete-room";
    test_for_delete(TEST_NAME, TEST_NAME, ErrorCode::RoomNotFound).await;
}

#[actix_rt::test]
async fn member() {
    const TEST_NAME: &str = "delete-member";
    test_for_delete(
        TEST_NAME,
        &format!("{}/publisher", TEST_NAME),
        ErrorCode::MemberNotFound,
    )
    .await;
}

#[actix_rt::test]
async fn endpoint() {
    const TEST_NAME: &str = "delete-endpoint";
    test_for_delete(
        &TEST_NAME,
        &format!("{}/publisher/publish", TEST_NAME),
        ErrorCode::EndpointNotFound,
    )
    .await;
}

/// Tests that `Delete` method on parent element also deletes all nested
/// elements.
///
/// # Arguments
///
/// `room_id`: `Room` ID which will be created and from will be deleted
/// `Element`s,
///
/// `elements_uris`: `Element`s IDs which will be deleted from this `Element`,
///
/// `error_code`: [`ErrorCode`] which should be returned from [`ControlAPI`]
/// when we tries get deleted `Element`,
///
/// `root_elem_uri`: URI to parent `Element`.
///
/// [Medea]: https://github.com/instrumentisto/medea
/// [Control API]: https://tinyurl.com/yxsqplq7
/// [`ErrorCode`]: medea::api::control::error_codes::ErrorCode
async fn test_cascade_delete(
    room_id: &str,
    elements_uris: &[&str],
    code: MedeaErrorCode,
    root_elem_uri: &str,
) {
    let mut client = ControlClient::new().await;
    client.create(create_room_req(room_id)).await;
    client.delete(elements_uris).await.unwrap();

    match client.try_get(root_elem_uri).await {
        Ok(_) => panic!("Member not deleted!"),
        Err(e) => {
            assert_eq!(e.code, code as u32);
        }
    }
}

#[actix_rt::test]
async fn cascade_delete_endpoints_when_deleting_member() {
    const TEST_NAME: &str = "member-and-endpoint-same-time";

    test_cascade_delete(
        TEST_NAME,
        &[
            &format!("{}/publisher", TEST_NAME),
            &format!("{}/publisher/publish", TEST_NAME),
        ],
        MedeaErrorCode::MemberNotFound,
        &format!("{}/publisher", TEST_NAME),
    )
    .await;
}

#[actix_rt::test]
async fn cascade_delete_everything_when_deleting_room() {
    const TEST_NAME: &str = "room-and-inner-elements-same-time";

    test_cascade_delete(
        TEST_NAME,
        &[
            &TEST_NAME,
            &format!("{}/publisher", TEST_NAME),
            &format!("{}/publisher/publish", TEST_NAME),
        ],
        MedeaErrorCode::RoomNotFound,
        TEST_NAME,
    )
    .await;
}

#[actix_rt::test]
async fn cant_delete_members_from_different_rooms_in_single_request() {
    let mut client = ControlClient::new().await;

    if let Err(err) = client.delete(&["room1/member1", "room2/member1"]).await {
        assert_eq!(err.code, MedeaErrorCode::ProvidedNotSameRoomIds as u32);
    } else {
        panic!("should err")
    }
}

#[actix_rt::test]
async fn cant_delete_endpoints_from_different_rooms_in_single_request() {
    let mut client = ControlClient::new().await;

    if let Err(err) = client
        .delete(&["room1/member1/endpoint1", "room2/member1/endpoint1"])
        .await
    {
        assert_eq!(err.code, MedeaErrorCode::ProvidedNotSameRoomIds as u32);
    } else {
        panic!("should err")
    }
}

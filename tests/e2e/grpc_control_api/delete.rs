//! Tests for `Delete` method of gRPC [Control API].
//!
//! The specificity of these tests is such that the `Get` method is also
//! being tested at the same time.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

use medea::api::control::error_codes::{
    ErrorCode as MedeaErrorCode, ErrorCode,
};

use crate::gen_insert_str_macro;

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
fn test_for_delete(
    room_id: &str,
    element_id: &str,
    error_code: MedeaErrorCode,
) {
    let client = ControlClient::new();
    client.create(&create_room_req(room_id));

    client.try_get(element_id).unwrap();

    client.delete(&[element_id]).unwrap();

    let get_room_err = match client.try_get(element_id) {
        Ok(_) => panic!("{} not deleted!", element_id),
        Err(e) => e,
    };
    assert_eq!(get_room_err.code, error_code as u32);
}

#[test]
fn room() {
    gen_insert_str_macro!("delete-room");
    test_for_delete(
        &insert_str!("{}"),
        &insert_str!("{}"),
        ErrorCode::RoomNotFound,
    );
}

#[test]
fn member() {
    gen_insert_str_macro!("delete-member");
    test_for_delete(
        &insert_str!("{}"),
        &insert_str!("{}/publisher"),
        ErrorCode::MemberNotFound,
    );
}

#[test]
fn endpoint() {
    gen_insert_str_macro!("delete-endpoint");
    test_for_delete(
        &insert_str!("{}"),
        &insert_str!("{}/publisher/publish"),
        ErrorCode::EndpointNotFound,
    );
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
fn test_cascade_delete(
    room_id: &str,
    elements_uris: &[&str],
    code: MedeaErrorCode,
    root_elem_uri: &str,
) {
    let client = ControlClient::new();
    client.create(&create_room_req(room_id));
    client.delete(elements_uris).unwrap();

    match client.try_get(root_elem_uri) {
        Ok(_) => panic!("Member not deleted!"),
        Err(e) => {
            assert_eq!(e.code, code as u32);
        }
    }
}

#[test]
fn cascade_delete_endpoints_when_deleting_member() {
    gen_insert_str_macro!("member-and-endpoint-same-time");

    test_cascade_delete(
        &insert_str!("{}"),
        &[
            &insert_str!("{}/publisher"),
            &insert_str!("{}/publisher/publish"),
        ],
        MedeaErrorCode::MemberNotFound,
        &insert_str!("{}/publisher"),
    );
}

#[test]
fn cascade_delete_everything_when_deleting_room() {
    gen_insert_str_macro!("room-and-inner-elements-same-time");

    test_cascade_delete(
        &insert_str!("{}"),
        &[
            &insert_str!("{}"),
            &insert_str!("{}/publisher"),
            &insert_str!("{}/publisher/publish"),
        ],
        MedeaErrorCode::RoomNotFound,
        &insert_str!("{}"),
    );
}

#[test]
fn cant_delete_members_from_different_rooms_in_single_request() {
    let client = ControlClient::new();

    if let Err(err) = client.delete(&["room1/member1", "room2/member1"]) {
        assert_eq!(err.code, MedeaErrorCode::ProvidedNotSameRoomIds as u32);
    } else {
        panic!("should err")
    }
}

#[test]
fn cant_delete_endpoints_from_different_rooms_in_single_request() {
    let client = ControlClient::new();

    if let Err(err) =
        client.delete(&["room1/member1/endpoint1", "room2/member1/endpoint1"])
    {
        assert_eq!(err.code, MedeaErrorCode::ProvidedNotSameRoomIds as u32);
    } else {
        panic!("should err")
    }
}

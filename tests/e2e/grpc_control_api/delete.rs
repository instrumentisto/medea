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
/// `error_code`: [`ErrorCode`] which should be returned from [ControlAPI] when
/// we tries get deleted `Element`.
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
    client.delete(&[element_id]);

    let get_room_err = match client.try_get(element_id) {
        Ok(_) => panic!("{} not deleted!", element_id),
        Err(e) => e,
    };
    assert_eq!(get_room_err.code, error_code as u32);
}

#[test]
fn room() {
    test_for_delete(
        "delete-room",
        "local://delete-room",
        ErrorCode::RoomNotFound,
    );
}

#[test]
fn member() {
    test_for_delete(
        "delete-member",
        "local://delete-member/publisher",
        ErrorCode::MemberNotFound,
    );
}

#[test]
fn endpoint() {
    test_for_delete(
        "delete-endpoint",
        "local://delete-endpoint/publisher/publish",
        ErrorCode::EndpointNotFound,
    );
}

/// Tests `Delete` method of [Control API] by trying to delete child `Element`
/// from the also deleting parent `Element`.
///
/// # Arguments
///
/// `room_id`: `Room` ID which will be created and from will be deleted
/// `Element`s,
///
/// `elements_uris`: `Element`s IDs which will be deleted from this `Element`,
///
/// `error_code`: [`ErrorCode`] which should be returned from [ControlAPI] when
/// we tries get deleted `Element`,
///
/// `root_elem_uri`: URI to parent `Element`.
///
/// [Medea]: https://github.com/instrumentisto/medea
/// [Control API]: https://tinyurl.com/yxsqplq7
/// [`ErrorCode`]: medea::api::control::error_codes::ErrorCode
fn test_for_delete_elements_at_same_time_test(
    room_id: &str,
    elements_uris: &[&str],
    code: MedeaErrorCode,
    root_elem_uri: &str,
) {
    let client = ControlClient::new();
    client.create(&create_room_req(room_id));
    client.delete(elements_uris);

    match client.try_get(root_elem_uri) {
        Ok(_) => panic!("Member not deleted!"),
        Err(e) => {
            assert_eq!(e.code, code as u32);
        }
    }
}

#[test]
fn member_and_endpoint_same_time() {
    test_for_delete_elements_at_same_time_test(
        "medea-and-endpoint-same-time",
        &[
            "local://medea-and-endpoint-same-time/publisher",
            "local://medea-and-endpoint-same-time/publisher/publish",
        ],
        MedeaErrorCode::MemberNotFound,
        "local://medea-and-endpoint-same-time/publisher",
    );
}

#[test]
fn room_and_inner_elements_same_time() {
    test_for_delete_elements_at_same_time_test(
        "room-and-inner-elements-same-time",
        &[
            "local://room-and-inner-elements-same-time",
            "local://room-and-inner-elements-same-time/publisher",
            "local://room-and-inner-elements-same-time/publisher/publish",
        ],
        MedeaErrorCode::RoomNotFound,
        "local://room-and-inner-elements-same-time",
    );
}

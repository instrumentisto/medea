use medea::api::error_codes::{ErrorCode as MedeaErrorCode, ErrorCode};

use super::{create_room_req, ControlClient};

fn delete_test(room_id: &str, element_id: &str, error_code: MedeaErrorCode) {
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
    delete_test(
        "delete-room",
        "local://delete-room",
        ErrorCode::RoomNotFound,
    );
}

#[test]
fn member() {
    delete_test(
        "delete-member",
        "local://delete-member/publisher",
        ErrorCode::MemberNotFound,
    );
}

#[test]
fn endpoint() {
    delete_test(
        "delete-endpoint",
        "local://delete-endpoint/publisher/publish",
        ErrorCode::EndpointNotFound,
    );
}

fn delete_elements_at_same_time_test(
    room_id: &str,
    elements_ids: &[&str],
    code: MedeaErrorCode,
    root_elem_id: &str,
) {
    let client = ControlClient::new();
    client.create(&create_room_req(room_id));
    client.delete(elements_ids);

    match client.try_get(root_elem_id) {
        Ok(_) => panic!("Member not deleted!"),
        Err(e) => {
            assert_eq!(e.code, code as u32);
        }
    }
}

#[test]
fn member_and_endpoint_same_time() {
    delete_elements_at_same_time_test(
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
    delete_elements_at_same_time_test(
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

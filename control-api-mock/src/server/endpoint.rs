use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct EndpointPath {
    room_id: String,
    member_id: String,
    endpoint_id: String,
}

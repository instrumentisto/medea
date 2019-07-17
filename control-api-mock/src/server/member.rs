use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct MemberPath {
    room_id: String,
    member_id: String,
}

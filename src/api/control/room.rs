//! Room definitions and implementations.

use serde::{Serialize, Deserialize};

use super::member::MemberRequest;

#[derive(Serialize, Deserialize, Debug)]
/// Spec of [`Room`]
pub struct RoomSpec {
    pipeline: RoomPipeline,
}

#[derive(Serialize, Deserialize, Debug)]
/// [`Room`] pipeline.
pub struct RoomPipeline {
    /// Caller [`Member`] spec
    caller: MemberRequest,
    /// Responder [`Member`] spec
    responder: MemberRequest,
}

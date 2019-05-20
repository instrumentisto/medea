//! Room definitions and implementations.

use serde::{Deserialize, Serialize};

use super::member::MemberRequest;

#[derive(Serialize, Deserialize, Debug, Clone)]
/// Spec of [`Room`]
pub struct RoomSpec {
    pipeline: RoomPipeline,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
/// [`Room`] pipeline.
pub struct RoomPipeline {
    /// Caller [`Member`] spec
    caller: MemberRequest,
    /// Responder [`Member`] spec
    responder: MemberRequest,
}

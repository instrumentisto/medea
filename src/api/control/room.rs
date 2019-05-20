//! Room definitions and implementations.

use serde::{Serialize, Deserialize};

use super::member::MemberKind;

#[derive(Serialize, Deserialize, Debug)]
/// Spec of [`Room`]
pub struct RoomSpec {
    pipeline: RoomPipeline,
}

#[derive(Serialize, Deserialize, Debug)]
/// [`Room`] pipeline.
pub struct RoomPipeline {
    /// Caller [`Member`] spec
    caller: MemberKind,
    /// Responder [`Member`] spec
    responder: MemberKind,
}

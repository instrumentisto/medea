//! Room definitions and implementations.

use serde::Deserialize;

use super::member::MemberRequest;

use std::collections::HashMap;

#[derive(Deserialize, Debug, Clone)]
/// Spec of [`Room`]
pub struct RoomSpec {
    pub pipeline: HashMap<String, MemberRequest>,
}

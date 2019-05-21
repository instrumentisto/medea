//! Room definitions and implementations.

use serde::{Deserialize, Serialize};

use super::member::MemberRequest;

use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
/// Spec of [`Room`]
pub struct RoomSpec {
    pub pipeline: HashMap<String, MemberRequest>,
}

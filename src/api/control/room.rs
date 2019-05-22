//! Room definitions and implementations.

use serde::Deserialize;

use super::member::MemberRequest;

use std::collections::HashMap;
use crate::api::control::MemberId;
use crate::api::control::member::MemberSpec;
use crate::api::control::element::Element;

#[derive(Deserialize, Debug, Clone)]
/// Spec of [`Room`]
pub struct RoomSpec {
    pub pipeline: HashMap<String, MemberRequest>,
}

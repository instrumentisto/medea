//! Member definitions and implementations.

use serde::Deserialize;

use super::element::Element;

use std::collections::HashMap;

/// ID of [`Member`].
pub type Id = u64;

/// Media server user with its ID and credentials.
#[derive(Debug, Clone)]
pub struct Member {
    /// ID of [`Member`].
    pub id: Id,

    /// Credentials to authorize [`Member`] with.
    pub credentials: String,

    pub spec: MemberSpec,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "kind")]
/// Entity for member requests.
pub enum MemberRequest {
    Member { spec: MemberSpec },
}

#[derive(Deserialize, Debug, Clone)]
/// Spec of member in [`Room`] pipeline.
pub struct MemberSpec {
    pub pipeline: HashMap<String, Element>,
}

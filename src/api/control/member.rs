//! Member definitions and implementations.

use serde::{Serialize, Deserialize};

use super::element::{PublishElement, PlayElement};

/// ID of [`Member`].
pub type Id = u64;

/// Media server user with its ID and credentials.
#[derive(Clone, Debug)]
pub struct Member {
    /// ID of [`Member`].
    pub id: Id,

    /// Credentials to authorize [`Member`] with.
    pub credentials: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "kind")]
/// Entity for member requests.
pub enum MemberRequest {
    Member {
        spec: MemberSpec,
    },
}

#[derive(Serialize, Deserialize, Debug)]
/// Spec of member in [`Room`] pipeline.
pub struct MemberSpec {
    pub pipeline: MemberPipeline,
}

#[derive(Serialize, Deserialize, Debug)]
/// Pipeline of [`Member`]
pub struct MemberPipeline {
    /// Publish pipeline of [`Member`]
    pub publish: PublishElement,
    /// Play pipeline of [`Member`]
    pub play: PlayElement,
}

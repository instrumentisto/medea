//! Member definitions and implementations.

use serde::Deserialize;
use std::{
    collections::HashMap,
    convert::TryFrom,
};

use super::{
    element::Element,
    pipeline::Pipeline,
    Entity,
    TryFromEntityError,
};

pub type Id = u64;

/// Media server user with its ID and credentials.
#[derive(Debug, Clone)]
pub struct Member {
    /// ID of [`Member`].
    pub id: Id,

    /// Credentials to authorize [`Member`] with.
    pub credentials: String,

    pub spec: MemberSpec,

    pub control_id: String,
}

#[derive(Clone, Debug)]
pub struct MemberSpec(pub Pipeline);
impl MemberSpec {
    pub fn get_element(
        &self,
        id: &str,
    ) -> Option<Result<Element, TryFromEntityError>> {
        Some(Element::try_from(self.0.pipeline.get(id).cloned()?))
    }
}

impl TryFrom<Entity> for MemberSpec {
    type Error = TryFromEntityError;

    fn try_from(from: Entity) -> Result<Self, Self::Error> {
        match from {
            Entity::Member { spec } => Ok(MemberSpec(spec)),
            _ => Err(TryFromEntityError::NotMember),
        }
    }
}

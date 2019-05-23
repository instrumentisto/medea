//! Room definitions and implementations.

use std::convert::TryFrom;

use super::{
    member::MemberSpec, pipeline::Pipeline, Entity, TryFromEntityError,
};

use crate::signalling::RoomId;

#[derive(Clone, Debug)]
pub struct RoomSpec {
    pub id: RoomId,
    pub spec: Pipeline,
}
impl RoomSpec {
    pub fn get_member(
        &self,
        id: &str,
    ) -> Option<Result<MemberSpec, TryFromEntityError>> {
        Some(MemberSpec::try_from(self.spec.pipeline.get(id).cloned()?))
    }
}

impl TryFrom<Entity> for RoomSpec {
    type Error = TryFromEntityError;

    fn try_from(from: Entity) -> Result<Self, Self::Error> {
        match from {
            Entity::Room { id, spec } => Ok(RoomSpec { id, spec }),
            _ => Err(TryFromEntityError::NotRoom),
        }
    }
}

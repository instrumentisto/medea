//! Room definitions and implementations.

use std::convert::TryFrom;

use super::{
    member::MemberSpec, pipeline::Pipeline, Entity, MemberId,
    TryFromEntityError,
};

/// ID of [`Room`].
pub type Id = String;

/// [`crate::signalling::room::Room`] specification.
/// Newtype for [`Entity::Room`]
#[derive(Clone, Debug)]
#[allow(clippy::module_name_repetitions)]
pub struct RoomSpec {
    pub id: Id,
    pub spec: Pipeline,
}

impl RoomSpec {
    /// Try to find [`MemberSpec`] by ID.
    ///
    /// Return `None` if [`MemberSpec`] not presented in [`RoomSpec`].
    /// Return `Some(TryFromEntityError::NotMember)` if entity with this ID
    ///         finded but its not [`MemberSpec`].
    #[allow(clippy::ptr_arg)]
    pub fn get_member(
        &self,
        id: &MemberId,
    ) -> Option<Result<MemberSpec, TryFromEntityError>> {
        Some(MemberSpec::try_from(self.spec.pipeline.get(id).cloned()?))
    }
}

impl TryFrom<Entity> for RoomSpec {
    type Error = TryFromEntityError;

    fn try_from(from: Entity) -> Result<Self, Self::Error> {
        match from {
            Entity::Room { id, spec } => Ok(Self { id, spec }),
            _ => Err(TryFromEntityError::NotRoom),
        }
    }
}

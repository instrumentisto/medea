//! Room definitions and implementations.

use hashbrown::HashMap;
use serde::Deserialize;
use std::convert::TryFrom;

use super::{
    element::Endpoint, member::MemberSpec, pipeline::Pipeline, Entity,
    MemberId, TryFromEntityError,
};

/// ID of [`Room`].
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq)]
pub struct Id(pub String);

/// [`crate::signalling::room::Room`] specification.
/// Newtype for [`Entity::Room`]
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug)]
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
    pub fn get_member(
        &self,
        id: &MemberId,
    ) -> Option<Result<MemberSpec, TryFromEntityError>> {
        Some(MemberSpec::try_from(
            self.spec.pipeline.get(&id.0).cloned()?,
        ))
    }

    /// Get all relations between [`Member`]s [`Endpoint`]s.
    ///
    /// Returns [`HashMap`] with [`MemberId`] of sender and all of his receivers
    /// [`MemberId`].
    ///
    /// Returns [`TryFromEntityError`] if some unexpected [`Entity`] finded.
    pub fn get_sender_receivers(
        &self,
    ) -> Result<HashMap<MemberId, Vec<MemberId>>, TryFromEntityError> {
        let mut sender_receivers: HashMap<MemberId, Vec<MemberId>> =
            HashMap::new();
        for (member_id, member_entity) in &self.spec.pipeline {
            let member_id = MemberId(member_id.clone());
            let member = MemberSpec::try_from(member_entity.clone())?;
            for element_entity in member.spec.pipeline.values() {
                let element = Endpoint::try_from(element_entity.clone())?;

                if let Endpoint::WebRtcPlay(play) = element {
                    if let Some(m) =
                        sender_receivers.get_mut(&play.src.member_id)
                    {
                        m.push(member_id.clone());
                    } else {
                        sender_receivers.insert(
                            play.src.member_id,
                            vec![member_id.clone()],
                        );
                    }
                }
            }
        }

        Ok(sender_receivers)
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

//! Room definitions and implementations.

use serde::Deserialize;
use std::convert::TryFrom;

use super::{
    member::MemberSpec, pipeline::Pipeline, Element,
    MemberId, TryFromElementError,
};

/// ID of [`Room`].
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq)]
pub struct Id(pub String);

/// [`crate::signalling::room::Room`] specification.
/// Newtype for [`Element::Room`]
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
    /// Return `Some(TryFromElementError::NotMember)` if element with this ID
    ///         finded but its not [`MemberSpec`].
    pub fn get_member(
        &self,
        id: &MemberId,
    ) -> Option<Result<MemberSpec, TryFromElementError>> {
        Some(MemberSpec::try_from(
            self.spec.pipeline.get(&id.0).cloned()?,
        ))
    }

    /// Get all receivers of all [`Member`]'s [`WebRtcPublishEndpoint`]s.
    pub fn get_receivers_for_member(&self, id: &MemberId) -> Result<Vec<MemberId>, TryFromElementError> {
        let mut receivers = Vec::new();
        for (member_id, member_element) in &self.spec.pipeline {
            let member = MemberSpec::try_from(member_element.clone())?;
            for endpoint in member.play_endpoints() {
                if &endpoint.src.member_id == id {
                    receivers.push(MemberId(member_id.clone()));
                }
            }
        }

        Ok(receivers)
    }
}

impl TryFrom<Element> for RoomSpec {
    type Error = TryFromElementError;

    fn try_from(from: Element) -> Result<Self, Self::Error> {
        match from {
            Element::Room { id, spec } => Ok(Self { id, spec }),
            _ => Err(TryFromElementError::NotRoom),
        }
    }
}

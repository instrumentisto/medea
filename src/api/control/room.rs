//! Room definitions and implementations.

use hashbrown::HashMap;
use serde::Deserialize;
use std::convert::TryFrom;

use super::{
    endpoint::Endpoint, member::MemberSpec, pipeline::Pipeline, Element,
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

    /// Get all relations between [`Member`]s [`Endpoint`]s.
    ///
    /// Returns [`HashMap`] with [`MemberId`] of sender and all of his receivers
    /// [`MemberId`].
    ///
    /// Returns [`TryFromElementError`] if some unexpected [`Element`] finded.
    pub fn get_sender_receivers(
        &self,
    ) -> Result<HashMap<MemberId, Vec<MemberId>>, TryFromElementError> {
        let mut sender_receivers: HashMap<MemberId, Vec<MemberId>> =
            HashMap::new();
        for (member_id, member_element) in &self.spec.pipeline {
            let member_id = MemberId(member_id.clone());
            let member = MemberSpec::try_from(member_element.clone())?;
            for endpoint_element in member.spec.pipeline.values() {
                let endpoint = Endpoint::try_from(endpoint_element.clone())?;

                if let Endpoint::WebRtcPlay(play) = endpoint {
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

impl TryFrom<Element> for RoomSpec {
    type Error = TryFromElementError;

    fn try_from(from: Element) -> Result<Self, Self::Error> {
        match from {
            Element::Room { id, spec } => Ok(Self { id, spec }),
            _ => Err(TryFromElementError::NotRoom),
        }
    }
}

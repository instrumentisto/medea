//! Room definitions and implementations.

use std::{collections::HashMap, convert::TryFrom};

use derive_more::{Display, From};
use serde::Deserialize;

use super::{
    member::{MemberElement, MemberSpec},
    pipeline::Pipeline,
    MemberId, RootElement, TryFromElementError,
};

/// ID of [`Room`].
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, From, Display)]
pub struct Id(pub String);

/// Element of [`Room`]'s [`Pipeline`].
///
/// [`Room`]: crate::signalling::room::Room
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Deserialize, Debug)]
#[serde(tag = "kind")]
pub enum RoomElement {
    /// Represent [`MemberSpec`].
    /// Can transform into [`MemberSpec`] by `MemberSpec::try_from`.
    Member {
        spec: Pipeline<MemberElement>,
        credentials: String,
    },
}

/// [`crate::signalling::room::Room`] specification.
/// Newtype for [`RootElement::Room`]
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug)]
pub struct RoomSpec {
    pub id: Id,
    pub pipeline: Pipeline<RoomElement>,
}

impl RoomSpec {
    /// Returns all [`MemberSpec`]s of this [`RoomSpec`].
    pub fn members(
        &self,
    ) -> Result<HashMap<MemberId, MemberSpec>, TryFromElementError> {
        let mut members: HashMap<MemberId, MemberSpec> = HashMap::new();
        for (control_id, element) in self.pipeline.iter() {
            let member_spec = MemberSpec::try_from(element)?;
            let member_id = MemberId(control_id.clone());

            members.insert(member_id.clone(), member_spec);
        }

        Ok(members)
    }

    /// Returns ID of this [`RoomSpec`]
    pub fn id(&self) -> &Id {
        &self.id
    }
}

impl TryFrom<&RootElement> for RoomSpec {
    type Error = TryFromElementError;

    // TODO: delete this allow when some new RootElement will be added.
    #[allow(unreachable_patterns)]
    fn try_from(from: &RootElement) -> Result<Self, Self::Error> {
        match from {
            RootElement::Room { id, spec } => Ok(Self {
                id: id.clone(),
                pipeline: spec.clone(),
            }),
            _ => Err(TryFromElementError::NotRoom),
        }
    }
}

//! Room definitions and implementations.

use std::{convert::TryFrom, sync::Arc};

use hashbrown::HashMap;
use serde::Deserialize;

use super::{
    member::MemberSpec, pipeline::Pipeline, Element, MemberId,
    TryFromElementError,
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
    pub pipeline: Arc<Pipeline>,
}

impl RoomSpec {
    /// Returns all [`Member`]s of this [`RoomSpec`].
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

impl TryFrom<&Element> for RoomSpec {
    type Error = TryFromElementError;

    fn try_from(from: &Element) -> Result<Self, Self::Error> {
        match from {
            Element::Room { id, spec } => Ok(Self {
                id: id.clone(),
                pipeline: Arc::new(spec.clone()),
            }),
            _ => Err(TryFromElementError::NotRoom),
        }
    }
}

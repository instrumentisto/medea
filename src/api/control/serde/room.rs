//! Room definitions and implementations.

use std::convert::TryFrom;

use hashbrown::HashMap;
use macro_attr::*;
use newtype_derive::{newtype_fmt, NewtypeDisplay, NewtypeFrom};
use serde::Deserialize;

use super::{
    member::SerdeMemberSpec, pipeline::Pipeline, Element, TryFromElementError,
};
use crate::api::control::model::{
    member::MemberSpec, room::RoomSpec, MemberId,
};

use crate::api::control::model::room::Id;

/// [`crate::signalling::room::Room`] specification.
/// Newtype for [`Element::Room`]
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug)]
pub struct SerdeRoomSpec {
    pub id: Id,
    pub pipeline: Pipeline,
}

impl SerdeRoomSpec {
    /// Returns all [`MemberSpec`]s of this [`RoomSpec`].
    pub fn members(
        &self,
    ) -> Result<HashMap<MemberId, SerdeMemberSpec>, TryFromElementError> {
        let mut members: HashMap<MemberId, SerdeMemberSpec> = HashMap::new();
        for (control_id, element) in self.pipeline.iter() {
            let member_spec = SerdeMemberSpec::try_from(element)?;
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

pub struct ParsedSerdeRoomSpec {
    id: Id,
    members: HashMap<MemberId, SerdeMemberSpec>,
}

impl ParsedSerdeRoomSpec {
    pub fn new(room_spec: &SerdeRoomSpec) -> Result<Self, TryFromElementError> {
        Ok(Self {
            id: room_spec.id.clone(),
            members: room_spec.members()?,
        })
    }
}

impl RoomSpec for ParsedSerdeRoomSpec {
    fn members(&self) -> HashMap<&MemberId, Box<&MemberSpec>> {
        self.members
            .iter()
            .map(|(id, member)| (id, Box::new(member as &MemberSpec)))
            .collect()
    }

    fn id(&self) -> &Id {
        &self.id
    }

    fn get_member_by_id(&self, id: &MemberId) -> Option<Box<&MemberSpec>> {
        self.members.get(id).map(|m| Box::new(m as &MemberSpec))
    }
}

impl TryFrom<&Element> for SerdeRoomSpec {
    type Error = TryFromElementError;

    fn try_from(from: &Element) -> Result<Self, Self::Error> {
        match from {
            Element::Room { id, spec } => Ok(Self {
                id: id.clone(),
                pipeline: spec.clone(),
            }),
            _ => Err(TryFromElementError::NotRoom),
        }
    }
}

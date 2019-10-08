//! Definitions and implementations of [Control API]'s `Room` element.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

use std::{collections::HashMap, convert::TryFrom};

use derive_more::{Display, From};
#[rustfmt::skip]
use medea_control_api_proto::grpc::api::{
    CreateRequest_oneof_el as ElementProto
};
use serde::Deserialize;

use crate::api::control::{EndpointId, TryFromProtobufError};

use super::{
    member::{MemberElement, MemberSpec},
    pipeline::Pipeline,
    MemberId, RootElement, TryFromElementError,
};

/// ID of [`Room`].
///
/// [`Room`]: crate::signalling::room::Room
#[derive(Clone, Debug, Deserialize, Display, Eq, Hash, PartialEq, From)]
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
        spec: Pipeline<EndpointId, MemberElement>,
        credentials: String,
    },
}

/// [Control API]'s `Room` element specification.
///
/// Newtype for [`RootElement::Room`].
///
/// [Control API]: https://tinyurl.com/yxsqplq7
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug)]
pub struct RoomSpec {
    pub id: Id,
    pub pipeline: Pipeline<MemberId, RoomElement>,
}

impl TryFrom<(Id, ElementProto)> for RoomSpec {
    type Error = TryFromProtobufError;

    fn try_from((id, proto): (Id, ElementProto)) -> Result<Self, Self::Error> {
        match proto {
            ElementProto::room(mut room) => {
                let mut pipeline = HashMap::new();
                for (id, room_element) in room.take_pipeline() {
                    if let Some(elem) = room_element.el {
                        let member =
                            MemberSpec::try_from((MemberId(id.clone()), elem))?;
                        pipeline.insert(id.into(), member.into());
                    } else {
                        return Err(TryFromProtobufError::EmptyElement(id));
                    }
                }

                let pipeline = Pipeline::new(pipeline);
                Ok(Self { id, pipeline })
            }
            _ => Err(TryFromProtobufError::ExpectedOtherElement(
                String::from("Room"),
                id.to_string(),
            )),
        }
    }
}

impl RoomSpec {
    /// Returns all [`MemberSpec`]s of this [`RoomSpec`].
    pub fn members(
        &self,
    ) -> Result<HashMap<MemberId, MemberSpec>, TryFromElementError> {
        let mut members: HashMap<MemberId, MemberSpec> = HashMap::new();
        for (control_id, element) in self.pipeline.iter() {
            let member_spec = MemberSpec::try_from(element)?;

            members.insert(control_id.clone(), member_spec);
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

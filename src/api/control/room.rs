//! Room definitions and implementations.

use std::{collections::HashMap as StdHashMap, convert::TryFrom};

use hashbrown::HashMap;
use macro_attr::*;
use newtype_derive::{newtype_fmt, NewtypeDisplay, NewtypeFrom};
use serde::Deserialize;

use crate::api::grpc::protos::control::Room as RoomProto;

use super::{
    member::MemberSpec, pipeline::Pipeline, Element, MemberId,
    TryFromElementError,
};
use crate::api::control::TryFromProtobufError;

macro_attr! {
    /// ID of [`Room`].
    #[derive(
        Clone,
        Debug,
        Deserialize,
        Eq,
        Hash,
        PartialEq,
        NewtypeFrom!,
        NewtypeDisplay!,
    )]
    pub struct Id(pub String);
}

/// [`crate::signalling::room::Room`] specification.
/// Newtype for [`Element::Room`]
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug)]
pub struct RoomSpec {
    pub id: Id,
    pub pipeline: Pipeline,
}

impl RoomSpec {
    pub fn try_from_protobuf(
        id: Id,
        proto: &RoomProto,
    ) -> Result<Self, TryFromProtobufError> {
        let mut pipeline = StdHashMap::new();
        for (id, room_element) in proto.get_pipeline() {
            if !room_element.has_member() {
                return Err(TryFromProtobufError::MemberElementNotFound);
            }
            let member = MemberSpec::try_from(room_element.get_member())?;
            // TODO: temporary
            let element = Element::Member {
                spec: member.pipeline,
                credentials: member.credentials,
            };
            pipeline.insert(id.clone(), element);
        }

        let pipeline = Pipeline::new(pipeline);

        Ok(Self { pipeline, id })
    }

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

// impl TryFrom<&RoomProto> for RoomSpec {
//    type Error = TryFromProtobufError;
//
//    fn try_from(value: &RoomProto) -> Result<Self, Self::Error> {
//        let mut pipeline = StdHashMap::new();
//        for (id, room_element) in value.get_pipeline() {
//            if !room_element.has_member() {
//                return Err(TryFromProtobufError::MemberElementNotFound);
//            }
//            let member = MemberSpec::try_from(room_element.get_member())?;
//            // TODO: temporary
//            let element = Element::Member {
//                spec: member.pipeline,
//                credentials: member.credentials,
//            };
//            pipeline.insert(id.clone(), element);
//        }
//
//        let pipeline = Pipeline::new(pipeline);
//
//        Ok(Self {
//            pipeline,
//            // TODO:
//            id: Id("unimplemented".to_string()),
//        })
//    }
//}

impl TryFrom<&Element> for RoomSpec {
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

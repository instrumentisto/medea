//! `Room` element related methods and entities.

use std::collections::HashMap;

use medea_control_api_proto::grpc::api::{
    Room as RoomProto, Room_Element as RoomElementProto,
    Room_Element_oneof_el as RoomElementOneOfEl,
};
use serde::{Deserialize, Serialize};

use super::member::Member;

/// [Control API]'s `Room` representation.
///
/// [Control API]: https://tinyurl.com/yxsqplq7
#[derive(Debug, Deserialize, Serialize)]
pub struct Room {
    /// ID of `Room`.
    #[serde(skip_deserializing)]
    id: String,

    /// Pipeline of `Room`.
    pipeline: HashMap<String, RoomElement>,
}

impl Room {
    /// Converts [`Room`] into protobuf [`RoomProto`].
    pub fn into_proto(self, id: String) -> RoomProto {
        let mut proto = RoomProto::new();
        let mut room_elements = HashMap::new();
        for (id, member) in self.pipeline {
            room_elements.insert(id.clone(), member.into_proto(id));
        }
        proto.set_id(id);
        proto.set_pipeline(room_elements);

        proto
    }
}

/// Element of [`Room`]'s pipeline.
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind")]
pub enum RoomElement {
    Member(Member),
}

impl RoomElement {
    pub fn into_proto(self, id: String) -> RoomElementProto {
        let mut proto = RoomElementProto::new();
        match self {
            Self::Member(m) => proto.set_member(m.into_proto(id)),
        }

        proto
    }
}

impl From<RoomElementProto> for RoomElement {
    fn from(proto: RoomElementProto) -> Self {
        match proto.el.unwrap() {
            RoomElementOneOfEl::member(member) => Self::Member(member.into()),
            _ => unimplemented!(),
        }
    }
}

impl From<RoomProto> for Room {
    fn from(mut proto: RoomProto) -> Self {
        let mut pipeline = HashMap::new();
        for (id, member) in proto.take_pipeline() {
            pipeline.insert(id, member.into());
        }
        let id = proto.take_id();
        Self { id, pipeline }
    }
}

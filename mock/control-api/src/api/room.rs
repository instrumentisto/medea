//! `Room` element related methods and entities.

use std::collections::HashMap;

use medea_control_api_proto::grpc::medea::{
    room::{element::El as RoomElementOneOfEl, Element as RoomElementProto},
    Room as RoomProto,
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
    #[must_use]
    pub fn into_proto(self, id: String) -> RoomProto {
        let room_elements = self
            .pipeline
            .into_iter()
            .map(|(id, member)| (id.clone(), member.into_proto(id)))
            .collect();

        RoomProto {
            id,
            pipeline: room_elements,
        }
    }
}

/// Element of [`Room`]'s pipeline.
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind")]
pub enum RoomElement {
    Member(Member),
}

impl RoomElement {
    #[must_use]
    pub fn into_proto(self, id: String) -> RoomElementProto {
        let el = match self {
            Self::Member(m) => RoomElementOneOfEl::Member(m.into_proto(id)),
        };

        RoomElementProto { el: Some(el) }
    }
}

impl From<RoomElementProto> for RoomElement {
    fn from(proto: RoomElementProto) -> Self {
        match proto.el.unwrap() {
            RoomElementOneOfEl::Member(member) => Self::Member(member.into()),
            _ => unimplemented!(),
        }
    }
}

impl From<RoomProto> for Room {
    fn from(proto: RoomProto) -> Self {
        let pipeline = proto
            .pipeline
            .into_iter()
            .map(|(id, member)| (id, member.into()))
            .collect();

        Self {
            id: proto.id,
            pipeline,
        }
    }
}

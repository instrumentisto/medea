//! `Room` element related methods and entities.

use std::collections::HashMap;

use medea_control_api_proto::grpc::api as proto;
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
    /// Converts [`Room`] into protobuf [`proto::Room`].
    #[must_use]
    pub fn into_proto(self, id: String) -> proto::Room {
        let room_elements = self
            .pipeline
            .into_iter()
            .map(|(id, member)| (id.clone(), member.into_proto(id)))
            .collect();

        proto::Room {
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
    pub fn into_proto(self, id: String) -> proto::room::Element {
        let el = match self {
            Self::Member(m) => {
                proto::room::element::El::Member(m.into_proto(id))
            }
        };

        proto::room::Element { el: Some(el) }
    }
}

impl From<proto::room::Element> for RoomElement {
    fn from(proto: proto::room::Element) -> Self {
        match proto.el.unwrap() {
            proto::room::element::El::Member(member) => {
                Self::Member(member.into())
            }
            _ => unimplemented!(),
        }
    }
}

impl From<proto::Room> for Room {
    fn from(proto: proto::Room) -> Self {
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

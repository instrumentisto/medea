//! `Member` element related methods and entities.

use std::collections::HashMap;

use medea_control_api_proto::grpc::medea::{
    room::{element::El as RoomElementOneOfProto, Element as RoomElementProto},
    Member as MemberProto,
};
use serde::{Deserialize, Serialize};

use super::endpoint::Endpoint;

/// Entity that represents [Control API] `Member`.
///
/// [Control API]: https://tinyurl.com/yxsqplq7
#[derive(Deserialize, Serialize, Debug)]
pub struct Member {
    /// ID of `Member`.
    #[serde(skip_deserializing)]
    id: String,

    /// Pipeline of [Control API] `Member`.
    ///
    /// [Control API]: https://tinyurl.com/yxsqplq7
    pipeline: HashMap<String, Endpoint>,

    /// Optional `Member` credentials.
    ///
    /// If `None` then random credentials will be generated on Medea side.
    credentials: Option<String>,

    /// URL to which `OnJoin` Control API callback will be sent.
    #[serde(skip_serializing_if = "Option::is_none")]
    on_join: Option<String>,

    /// URL to which `OnLeave` Control API callback will be sent.
    #[serde(skip_serializing_if = "Option::is_none")]
    on_leave: Option<String>,
}

impl Member {
    /// Converts [`Member`] into protobuf [`MemberProto`].
    #[must_use]
    pub fn into_proto(self, id: String) -> MemberProto {
        let member_elements = self
            .pipeline
            .into_iter()
            .map(|(id, endpoint)| (id.clone(), endpoint.into_proto(id)))
            .collect();

        MemberProto {
            pipeline: member_elements,
            id,
            credentials: self.credentials.unwrap_or_default(),
            on_join: self.on_join.unwrap_or_default(),
            on_leave: self.on_leave.unwrap_or_default(),
        }
    }

    /// Converts [`Member`] into protobuf [`RoomElementProto`].
    #[must_use]
    pub fn into_room_el_proto(self, id: String) -> RoomElementProto {
        RoomElementProto {
            el: Some(RoomElementOneOfProto::Member(self.into_proto(id))),
        }
    }
}

impl From<MemberProto> for Member {
    fn from(proto: MemberProto) -> Self {
        let member_pipeline = proto
            .pipeline
            .into_iter()
            .map(|(id, endpoint)| (id, endpoint.into()))
            .collect();

        Self {
            id: proto.id,
            pipeline: member_pipeline,
            credentials: Some(proto.credentials),
            on_join: Some(proto.on_join).filter(|s| !s.is_empty()),
            on_leave: Some(proto.on_leave).filter(|s| !s.is_empty()),
        }
    }
}

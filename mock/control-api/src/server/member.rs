//! `Member` element related methods and entities.

use std::collections::HashMap;

use medea_control_api_proto::grpc::api::{
    Member as MemberProto, Room_Element as RoomElementProto,
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

    #[serde(skip_serializing_if = "Option::is_none")]
    on_join: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    on_leave: Option<String>,
}

impl Member {
    /// Converts [`Member`] into protobuf [`MemberProto`].
    pub fn into_proto(self, id: String) -> MemberProto {
        let mut proto = MemberProto::new();
        let mut members_elements = HashMap::new();
        for (id, endpoint) in self.pipeline {
            members_elements.insert(id.clone(), endpoint.into_proto(id));
        }
        proto.set_id(id);
        proto.set_pipeline(members_elements);
        self.on_leave.map(|on_leave| proto.set_on_leave(on_leave));
        self.on_join.map(|on_join| proto.set_on_join(on_join));

        if let Some(credentials) = self.credentials {
            proto.set_credentials(credentials);
        }

        proto
    }

    /// Converts [`Member`] into protobuf [`RoomElementProto`].
    pub fn into_room_el_proto(self, id: String) -> RoomElementProto {
        let mut proto = RoomElementProto::new();
        proto.set_member(self.into_proto(id));
        proto
    }
}

impl From<MemberProto> for Member {
    fn from(mut proto: MemberProto) -> Self {
        let mut member_pipeline = HashMap::new();
        for (id, endpoint) in proto.take_pipeline() {
            member_pipeline.insert(id, endpoint.into());
        }

        let on_leave = {
            let on_leave = proto.take_on_leave();
            if on_leave.is_empty() {
                None
            } else {
                Some(on_leave)
            }
        };
        let on_join = {
            let on_join = proto.take_on_join();
            if on_join.is_empty() {
                None
            } else {
                Some(on_join)
            }
        };

        Self {
            id: proto.take_id(),
            pipeline: member_pipeline,
            credentials: Some(proto.take_credentials()),
            on_join,
            on_leave,
        }
    }
}

//! `Member` element related methods and entities.

use std::collections::HashMap;

use medea_control_api_proto::grpc::api as proto;
use serde::{Deserialize, Serialize};

use super::endpoint::Endpoint;
use std::time::Duration;

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

    #[serde(default)]
    #[serde(with = "humantime_serde")]
    idle_timeout: Duration,

    #[serde(default)]
    #[serde(with = "humantime_serde")]
    reconnect_timeout: Duration,

    #[serde(default)]
    #[serde(with = "humantime_serde")]
    ping_interval: Duration,
}

impl Member {
    /// Converts [`Member`] into protobuf [`proto::Member`].
    #[must_use]
    pub fn into_proto(self, id: String) -> proto::Member {
        let member_elements = self
            .pipeline
            .into_iter()
            .map(|(id, endpoint)| (id.clone(), endpoint.into_proto(id)))
            .collect();

        proto::Member {
            pipeline: member_elements,
            id,
            credentials: self.credentials.unwrap_or_default(),
            on_join: self.on_join.unwrap_or_default(),
            on_leave: self.on_leave.unwrap_or_default(),
            idle_timeout: self.idle_timeout.as_secs(),
            reconnect_timeout: self.reconnect_timeout.as_secs(),
            ping_interval: self.ping_interval.as_secs(),
        }
    }

    /// Converts [`Member`] into protobuf [`proto::room::Element`].
    #[must_use]
    pub fn into_room_el_proto(self, id: String) -> proto::room::Element {
        proto::room::Element {
            el: Some(proto::room::element::El::Member(self.into_proto(id))),
        }
    }
}

impl From<proto::Member> for Member {
    fn from(proto: proto::Member) -> Self {
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
            idle_timeout: Duration::from_secs(proto.idle_timeout),
            reconnect_timeout: Duration::from_secs(proto.reconnect_timeout),
            ping_interval: Duration::from_secs(proto.ping_interval),
        }
    }
}

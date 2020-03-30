//! `Member` element related methods and entities.

use std::{collections::HashMap, convert::TryInto as _, time::Duration};

use medea_control_api_proto::grpc::api as proto;
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

    /// [`Duration`], after which remote RPC client will be considered idle if
    /// no heartbeat messages received.
    #[serde(default)]
    #[serde(with = "humantime_serde")]
    idle_timeout: Option<Duration>,

    /// [`Duration`], after which the server deletes the client session if
    /// the remote RPC client does not reconnect after it is idle.
    #[serde(default)]
    #[serde(with = "humantime_serde")]
    reconnect_timeout: Option<Duration>,

    /// Interval of sending `Ping`s from the server to the client.
    #[serde(default)]
    #[serde(with = "humantime_serde")]
    ping_interval: Option<Duration>,
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
            idle_timeout: self.idle_timeout.map(Into::into),
            reconnect_timeout: self.reconnect_timeout.map(Into::into),
            ping_interval: self.ping_interval.map(Into::into),
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
            idle_timeout: proto.idle_timeout.map(|dur| dur.try_into().unwrap()),
            reconnect_timeout: proto
                .reconnect_timeout
                .map(|dur| dur.try_into().unwrap()),
            ping_interval: proto
                .ping_interval
                .map(|dur| dur.try_into().unwrap()),
        }
    }
}

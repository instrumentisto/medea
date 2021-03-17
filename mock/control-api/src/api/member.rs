//! `Member` element related methods and entities.

use std::{collections::HashMap, convert::TryInto as _, time::Duration};

use medea_control_api_proto::grpc::api as proto;
use serde::{Deserialize, Serialize};

use super::endpoint::Endpoint;

/// Entity that represents a [Control API] [`Member`].
///
/// [Control API]: https://tinyurl.com/yxsqplq7
#[derive(Deserialize, Serialize, Debug)]
pub struct Member {
    /// ID of this [`Member`].
    #[serde(skip_deserializing)]
    pub id: String,

    /// [Control API] pipeline of this [`Member`].
    ///
    /// [Control API]: https://tinyurl.com/yxsqplq7
    pub pipeline: HashMap<String, Endpoint>,

    /// Optional credentials of this [`Member`].
    ///
    /// If [`None`] then random credentials will be generated on Medea side.
    pub credentials: Option<Credentials>,

    /// URL to which `OnJoin` Control API callback will be sent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_join: Option<String>,

    /// URL to which `OnLeave` Control API callback will be sent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_leave: Option<String>,

    /// Timeout of receiving heartbeat messages from this [`Member`] via Client
    /// API. Once reached, the [`Member`] is considered being idle.
    #[serde(default, with = "humantime_serde")]
    pub idle_timeout: Option<Duration>,

    /// Timeout of this [`Member`] reconnecting via Client API.
    /// Once reached, the [`Member`] is considered disconnected.
    #[serde(default, with = "humantime_serde")]
    pub reconnect_timeout: Option<Duration>,

    /// Interval of sending pings from Medea to this [`Member`] via Client API.
    #[serde(default, with = "humantime_serde")]
    pub ping_interval: Option<Duration>,
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
            credentials: self.credentials.map(Into::into),
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
            credentials: proto.credentials.map(Into::into),
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

/// Credentials of the [`Member`].
#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Credentials {
    /// [Argon2] hash of the [`Member`] credentials.
    ///
    /// [Argon2]: https://en.wikipedia.org/wiki/Argon2
    Hash(String),

    /// Plain text [`Member`] credentials.
    Plain(String),
}

impl From<proto::member::Credentials> for Credentials {
    #[inline]
    fn from(from: proto::member::Credentials) -> Self {
        use proto::member::Credentials as C;
        match from {
            C::Plain(plain) => Self::Plain(plain),
            C::Hash(hash) => Self::Hash(hash),
        }
    }
}

impl From<Credentials> for proto::member::Credentials {
    #[inline]
    fn from(from: Credentials) -> Self {
        use Credentials as C;
        match from {
            C::Hash(hash) => Self::Hash(hash),
            C::Plain(plain) => Self::Plain(plain),
        }
    }
}

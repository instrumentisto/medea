//! Definitions and implementations of [Control API]'s `Member` element.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

use std::{
    collections::HashMap,
    convert::{TryFrom, TryInto},
    time::Duration,
};

use derive_more::{Display, From};
use medea_control_api_proto::grpc::api as proto;
use rand::{distributions::Alphanumeric, Rng};
use serde::Deserialize;

use crate::api::control::{
    callback::url::CallbackUrl,
    endpoints::{
        webrtc_play_endpoint::WebRtcPlayEndpoint,
        webrtc_publish_endpoint::{WebRtcPublishEndpoint, WebRtcPublishId},
    },
    pipeline::Pipeline,
    room::RoomElement,
    EndpointId, EndpointSpec, TryFromElementError, TryFromProtobufError,
    WebRtcPlayId,
};

const CREDENTIALS_LEN: usize = 32;

/// ID of `Member`.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, From, Display)]
#[from(forward)]
pub struct Id(pub String);

/// Element of [`Member`]'s [`Pipeline`].
///
/// [`Member`]: crate::signalling::elements::member::Member
#[derive(Clone, Deserialize, Debug)]
#[serde(tag = "kind")]
pub enum MemberElement {
    /// Represent [`WebRtcPublishEndpoint`].
    /// Can transform into [`EndpointSpec`] enum by `EndpointSpec::try_from`.
    ///
    /// [`EndpointSpec`]: crate::api::control::endpoints::EndpointSpec
    WebRtcPublishEndpoint { spec: WebRtcPublishEndpoint },

    /// Represent [`WebRtcPlayEndpoint`].
    /// Can transform into [`EndpointSpec`] enum by `EndpointSpec::try_from`.
    ///
    /// [`EndpointSpec`]: crate::api::control::endpoints::EndpointSpec
    WebRtcPlayEndpoint { spec: WebRtcPlayEndpoint },
}

/// Newtype for [`RoomElement::Member`] variant.
#[derive(Clone, Debug)]
pub struct MemberSpec {
    /// Spec of this `Member`.
    pipeline: Pipeline<EndpointId, MemberElement>,

    /// Credentials to authorize `Member` with.
    credentials: String,

    /// URL to which `OnJoin` Control API callback will be sent.
    on_join: Option<CallbackUrl>,

    /// URL to which `OnLeave` Control API callback will be sent.
    on_leave: Option<CallbackUrl>,

    /// Timeout of receiving heartbeat messages from the `Member` via Client
    /// API.
    ///
    /// Once reached, the `Member` is considered being idle.
    idle_timeout: Option<Duration>,

    /// Timeout of the `Member` reconnecting via Client API.
    ///
    /// Once reached, the `Member` is considered disconnected.
    reconnect_timeout: Option<Duration>,

    /// Interval of sending `Ping`s to the `Member` via Client API.
    ping_interval: Option<Duration>,
}

impl Into<RoomElement> for MemberSpec {
    fn into(self) -> RoomElement {
        RoomElement::Member {
            spec: self.pipeline,
            credentials: self.credentials,
            on_join: self.on_join,
            on_leave: self.on_leave,
            idle_timeout: self.idle_timeout,
            reconnect_timeout: self.reconnect_timeout,
            ping_interval: self.ping_interval,
        }
    }
}

impl MemberSpec {
    /// Creates new [`MemberSpec`] with the given parameters.
    #[inline]
    pub fn new(
        pipeline: Pipeline<EndpointId, MemberElement>,
        credentials: String,
        on_join: Option<CallbackUrl>,
        on_leave: Option<CallbackUrl>,
        idle_timeout: Option<Duration>,
        reconnect_timeout: Option<Duration>,
        ping_interval: Option<Duration>,
    ) -> Self {
        Self {
            pipeline,
            credentials,
            on_join,
            on_leave,
            idle_timeout,
            reconnect_timeout,
            ping_interval,
        }
    }

    /// Returns all [`WebRtcPlayEndpoint`]s of this [`MemberSpec`].
    pub fn play_endpoints(
        &self,
    ) -> impl Iterator<Item = (WebRtcPlayId, &WebRtcPlayEndpoint)> {
        self.pipeline.iter().filter_map(|(id, e)| match e {
            MemberElement::WebRtcPlayEndpoint { spec } => {
                Some((id.clone().into(), spec))
            }
            _ => None,
        })
    }

    /// Lookups [`WebRtcPublishEndpoint`] by ID.
    pub fn get_publish_endpoint_by_id(
        &self,
        id: WebRtcPublishId,
    ) -> Option<&WebRtcPublishEndpoint> {
        let e = self.pipeline.get(&id.into())?;
        if let MemberElement::WebRtcPublishEndpoint { spec } = e {
            Some(spec)
        } else {
            None
        }
    }

    /// Returns all [`WebRtcPublishEndpoint`]s of this [`MemberSpec`].
    pub fn publish_endpoints(
        &self,
    ) -> impl Iterator<Item = (WebRtcPublishId, &WebRtcPublishEndpoint)> {
        self.pipeline.iter().filter_map(|(id, e)| match e {
            MemberElement::WebRtcPublishEndpoint { spec } => {
                Some((id.clone().into(), spec))
            }
            _ => None,
        })
    }

    /// Returns credentials from this [`MemberSpec`].
    pub fn credentials(&self) -> &str {
        &self.credentials
    }

    /// Returns reference to `on_join` [`CallbackUrl`].
    pub fn on_join(&self) -> &Option<CallbackUrl> {
        &self.on_join
    }

    /// Returns reference to `on_leave` [`CallbackUrl`].
    pub fn on_leave(&self) -> &Option<CallbackUrl> {
        &self.on_leave
    }

    /// Returns timeout of receiving heartbeat messages from the `Member` via
    /// Client API.
    ///
    /// Once reached, the `Member` is considered being idle.
    pub fn idle_timeout(&self) -> Option<Duration> {
        self.idle_timeout
    }

    /// Returns timeout of the `Member` reconnecting via Client API.
    ///
    /// Once reached, the `Member` is considered disconnected.
    pub fn reconnect_timeout(&self) -> Option<Duration> {
        self.reconnect_timeout
    }

    /// Returns interval of sending `Ping`s to the `Member` via Client API.
    pub fn ping_interval(&self) -> Option<Duration> {
        self.ping_interval
    }
}

/// Generates alphanumeric credentials for [`Member`] with
/// [`CREDENTIALS_LEN`] length.
///
/// This credentials will be generated if in dynamic [Control API] spec not
/// provided credentials for [`Member`]. This logic you can find in [`TryFrom`]
/// [`MemberProto`] implemented for [`MemberSpec`].
///
/// [Control API]: https://tinyurl.com/yxsqplq7
fn generate_member_credentials() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(CREDENTIALS_LEN)
        .collect()
}

impl TryFrom<proto::Member> for MemberSpec {
    type Error = TryFromProtobufError;

    fn try_from(member: proto::Member) -> Result<Self, Self::Error> {
        fn parse_duration<T: TryInto<Duration>>(
            duration: Option<T>,
            member_id: &str,
            field: &'static str,
        ) -> Result<Option<Duration>, TryFromProtobufError> {
            duration.map(TryInto::try_into).transpose().map_err(|_| {
                TryFromProtobufError::NegativeDuration(member_id.into(), field)
            })
        }

        let mut pipeline = HashMap::new();
        for (id, member_element) in member.pipeline {
            if let Some(elem) = member_element.el {
                let endpoint = EndpointSpec::try_from((
                    EndpointId::from(id.clone()),
                    elem,
                ))?;
                pipeline.insert(id.into(), endpoint.into());
            } else {
                return Err(TryFromProtobufError::EmptyElement(id));
            }
        }

        let mut credentials = member.credentials;
        if credentials.is_empty() {
            credentials = generate_member_credentials();
        }

        let on_leave =
            member.on_leave.map(CallbackUrl::try_from).transpose()?;
        let on_join = member.on_join.map(CallbackUrl::try_from).transpose()?;

        let idle_timeout =
            parse_duration(member.idle_timeout, &member.id, "idle_timeout")?;
        let reconnect_timeout = parse_duration(
            member.reconnect_timeout,
            &member.id,
            "reconnect_timeout",
        )?;
        let ping_interval =
            parse_duration(member.ping_interval, &member.id, "ping_interval")?;

        Ok(Self {
            pipeline: Pipeline::new(pipeline),
            credentials,
            on_join,
            on_leave,
            idle_timeout,
            reconnect_timeout,
            ping_interval,
        })
    }
}

macro_rules! impl_try_from_proto_for_member {
    ($proto:path) => {
        impl TryFrom<(Id, $proto)> for MemberSpec {
            type Error = TryFromProtobufError;

            fn try_from(
                (id, proto): (Id, $proto),
            ) -> Result<Self, Self::Error> {
                use $proto as proto_el;
                match proto {
                    proto_el::Member(member) => Self::try_from(member),
                    _ => Err(TryFromProtobufError::ExpectedOtherElement(
                        String::from("Member"),
                        id.to_string(),
                    )),
                }
            }
        }
    };
}

impl_try_from_proto_for_member!(proto::room::element::El);
impl_try_from_proto_for_member!(proto::create_request::El);

impl TryFrom<&RoomElement> for MemberSpec {
    type Error = TryFromElementError;

    // TODO: delete this allow when some new RoomElement will be added.
    #[allow(unreachable_patterns)]
    fn try_from(from: &RoomElement) -> Result<Self, Self::Error> {
        match from {
            RoomElement::Member {
                spec,
                credentials,
                on_leave,
                on_join,
                idle_timeout,
                reconnect_timeout,
                ping_interval,
            } => Ok(Self {
                pipeline: spec.clone(),
                credentials: credentials.clone(),
                on_leave: on_leave.clone(),
                on_join: on_join.clone(),
                idle_timeout: *idle_timeout,
                reconnect_timeout: *reconnect_timeout,
                ping_interval: *ping_interval,
            }),
            _ => Err(TryFromElementError::NotMember),
        }
    }
}

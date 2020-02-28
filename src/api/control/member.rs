//! Definitions and implementations of [Control API]'s `Member` element.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

use std::{collections::HashMap, convert::TryFrom};

use derive_more::{Display, From};
use medea_control_api_proto::grpc::api as proto;
use rand::{distributions::Alphanumeric, Rng};
use serde::Deserialize;

use crate::api::control::{
    callback::url::CallbackUrl,
    endpoints::{
        webrtc_play_endpoint::{
            Unvalidated, Validated, ValidationError, WebRtcPlayEndpoint,
        },
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
pub struct Id(pub String);

/// Element of [`Member`]'s [`Pipeline`].
///
/// [`Member`]: crate::signalling::elements::member::Member
#[derive(Clone, Deserialize, Debug)]
#[serde(tag = "kind")]
pub enum MemberElement<T> {
    /// Represent [`WebRtcPublishEndpoint`].
    /// Can transform into [`EndpointSpec`] enum by `EndpointSpec::try_from`.
    ///
    /// [`EndpointSpec`]: crate::api::control::endpoints::EndpointSpec
    #[serde(bound = "T: From<Unvalidated> + Default")]
    WebRtcPublishEndpoint { spec: WebRtcPublishEndpoint<T> },

    /// Represent [`WebRtcPlayEndpoint`].
    /// Can transform into [`EndpointSpec`] enum by `EndpointSpec::try_from`.
    ///
    /// [`EndpointSpec`]: crate::api::control::endpoints::EndpointSpec
    #[serde(bound = "T: From<Unvalidated>")]
    WebRtcPlayEndpoint { spec: WebRtcPlayEndpoint<T> },
}

impl MemberElement<Unvalidated> {
    pub fn validate(self) -> Result<MemberElement<Validated>, ValidationError> {
        match self {
            MemberElement::WebRtcPublishEndpoint { spec } => {
                Ok(MemberElement::WebRtcPublishEndpoint {
                    spec: spec.validate()?,
                })
            }
            MemberElement::WebRtcPlayEndpoint { spec } => {
                Ok(MemberElement::WebRtcPlayEndpoint {
                    spec: spec.validate()?,
                })
            }
        }
    }
}

/// Newtype for [`RoomElement::Member`] variant.
#[derive(Clone, Debug)]
pub struct MemberSpec {
    /// Spec of this `Member`.
    pipeline: Pipeline<EndpointId, MemberElement<Validated>>,

    /// Credentials to authorize `Member` with.
    credentials: String,

    /// URL to which `OnJoin` Control API callback will be sent.
    on_join: Option<CallbackUrl>,

    /// URL to which `OnLeave` Control API callback will be sent.
    on_leave: Option<CallbackUrl>,
}

impl Into<RoomElement<Validated>> for MemberSpec {
    fn into(self) -> RoomElement<Validated> {
        RoomElement::Member {
            spec: self.pipeline,
            credentials: self.credentials,
            on_join: self.on_join,
            on_leave: self.on_leave,
        }
    }
}

type PublishEndpointsItem<'a> =
    (WebRtcPublishId, &'a WebRtcPublishEndpoint<Validated>);

impl MemberSpec {
    /// Returns all [`WebRtcPlayEndpoint`]s of this [`MemberSpec`].
    pub fn play_endpoints(
        &self,
    ) -> impl Iterator<Item = (WebRtcPlayId, &WebRtcPlayEndpoint<Validated>)>
    {
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
    ) -> Option<&WebRtcPublishEndpoint<Validated>> {
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
    ) -> impl Iterator<Item = PublishEndpointsItem> {
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
        let mut pipeline = HashMap::new();
        for (id, member_element) in member.pipeline {
            if let Some(elem) = member_element.el {
                let endpoint =
                    EndpointSpec::try_from((EndpointId(id.clone()), elem))?;
                pipeline.insert(id.into(), endpoint.into());
            } else {
                return Err(TryFromProtobufError::EmptyElement(id));
            }
        }

        let mut credentials = member.credentials;
        if credentials.is_empty() {
            credentials = generate_member_credentials();
        }

        let on_leave = {
            let on_leave = member.on_leave;
            if on_leave.is_empty() {
                None
            } else {
                Some(CallbackUrl::try_from(on_leave)?)
            }
        };
        let on_join = {
            let on_join = member.on_join;
            if on_join.is_empty() {
                None
            } else {
                Some(CallbackUrl::try_from(on_join)?)
            }
        };

        Ok(Self {
            pipeline: Pipeline::new(pipeline),
            credentials,
            on_join,
            on_leave,
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

impl TryFrom<&RoomElement<Validated>> for MemberSpec {
    type Error = TryFromElementError;

    // TODO: delete this allow when some new RoomElement will be added.
    #[allow(unreachable_patterns)]
    fn try_from(from: &RoomElement<Validated>) -> Result<Self, Self::Error> {
        match from {
            RoomElement::Member {
                spec,
                credentials,
                on_leave,
                on_join,
            } => Ok(Self {
                pipeline: spec.clone(),
                credentials: credentials.clone(),
                on_leave: on_leave.clone(),
                on_join: on_join.clone(),
            }),
            _ => Err(TryFromElementError::NotMember),
        }
    }
}

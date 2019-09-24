//! Definitions and implementations of [Control API]'s `Member` element.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

use std::{collections::HashMap, convert::TryFrom};

use derive_more::{Display, From};
use medea_control_api_proto::grpc::control_api::{
    CreateRequest_oneof_el as ElementProto,
    Room_Element_oneof_el as RoomElementProto,
};
use rand::{distributions::Alphanumeric, Rng};
use serde::Deserialize;

use crate::api::control::{
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
pub struct Id(pub String);

/// Element of [`Member`]'s [`Pipeline`].
///
/// [`Member`]: crate::signalling::elements::member::Member
#[allow(clippy::module_name_repetitions)]
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
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug)]
pub struct MemberSpec {
    // TODO: add id
    /// Spec of this `Member`.
    pipeline: Pipeline<EndpointId, MemberElement>,

    /// Credentials to authorize `Member` with.
    credentials: String,
}

impl Into<RoomElement> for MemberSpec {
    fn into(self) -> RoomElement {
        RoomElement::Member {
            spec: self.pipeline,
            credentials: self.credentials,
        }
    }
}

impl MemberSpec {
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
}

/// Generates alphanumeric credentials for [`Member`] with
/// [`CREDENTIALS_LEN`] length.
///
/// This credentials will be generated if in dynamic [Control API] spec not
/// provided credentials for [`Member`]. This logic you can find in [`TryFrom`]
/// [`MemberProto`] implemented for [`MemberSpec`].
fn generate_member_credentials() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(CREDENTIALS_LEN)
        .collect()
}

macro_rules! impl_try_from_proto_for_member {
    ($proto:tt) => {
        impl TryFrom<(Id, $proto)> for MemberSpec {
            type Error = TryFromProtobufError;

            fn try_from(value: (Id, $proto)) -> Result<Self, Self::Error> {
                let id = value.0;
                let proto = value.1;

                match proto {
                    $proto::member(mut member) => {
                        let mut pipeline = HashMap::new();
                        for (id, member_element) in member.take_pipeline() {
                            if let Some(elem) = member_element.el {
                                let endpoint = EndpointSpec::try_from((
                                    EndpointId(id.clone()),
                                    elem,
                                ))?;
                                pipeline.insert(id.into(), endpoint.into());
                            } else {
                                return Err(
                                    TryFromProtobufError::EmptyElement(id),
                                );
                            }
                        }

                        let mut credentials = member.take_credentials();
                        if credentials.is_empty() {
                            credentials = generate_member_credentials();
                        }

                        Ok(Self {
                            pipeline: Pipeline::new(pipeline),
                            credentials,
                        })
                    }
                    _ => Err(TryFromProtobufError::ExpectedOtherElement(
                        String::from("Member"),
                        id.to_string(),
                    )),
                }
            }
        }
    };
}

impl_try_from_proto_for_member!(RoomElementProto);
impl_try_from_proto_for_member!(ElementProto);

impl TryFrom<&RoomElement> for MemberSpec {
    type Error = TryFromElementError;

    // TODO: delete this allow when some new RoomElement will be added.
    #[allow(unreachable_patterns)]
    fn try_from(from: &RoomElement) -> Result<Self, Self::Error> {
        match from {
            RoomElement::Member { spec, credentials } => Ok(Self {
                pipeline: spec.clone(),
                credentials: credentials.clone(),
            }),
            _ => Err(TryFromElementError::NotMember),
        }
    }
}

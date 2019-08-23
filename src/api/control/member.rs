//! Member definitions and implementations.

use std::{collections::HashMap as StdHashMap, convert::TryFrom};

use macro_attr::*;
use medea_grpc_proto::control::Member as MemberProto;
use newtype_derive::{newtype_fmt, NewtypeDisplay, NewtypeFrom};
use rand::{distributions::Alphanumeric, Rng};
use serde::Deserialize;

use crate::api::control::{
    endpoints::{
        webrtc_play_endpoint::WebRtcPlayEndpoint,
        webrtc_publish_endpoint::{WebRtcPublishEndpoint, WebRtcPublishId},
    },
    pipeline::Pipeline,
    room::RoomElement,
    Endpoint, TryFromElementError, TryFromProtobufError, WebRtcPlayId,
};

const MEMBER_CREDENTIALS_LEN: usize = 32;

macro_attr! {
    /// ID of [`Member`].
    #[derive(
        Clone,
        Debug,
        Deserialize,
        Eq,
        Hash,
        PartialEq,
        NewtypeFrom!,
        NewtypeDisplay!,
    )]
    pub struct Id(pub String);
}

/// Element of [`Member`]'s [`Pipeline`].
///
/// [`Member`]: crate::signalling::elements::member::Member
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Deserialize, Debug)]
#[serde(tag = "kind")]
pub enum MemberElement {
    /// Represent [`WebRtcPublishEndpoint`].
    /// Can transform into [`Endpoint`] enum by `Endpoint::try_from`.
    ///
    /// [`Endpoint`]: crate::api::control::endpoint::Endpoint
    WebRtcPublishEndpoint { spec: WebRtcPublishEndpoint },

    /// Represent [`WebRtcPlayEndpoint`].
    /// Can transform into [`Endpoint`] enum by `Endpoint::try_from`.
    ///
    /// [`Endpoint`]: crate::api::control::endpoint::Endpoint
    WebRtcPlayEndpoint { spec: WebRtcPlayEndpoint },
}

/// Newtype for [`RoomElement::Member`] variant.
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug)]
pub struct MemberSpec {
    /// Spec of this `Member`.
    pipeline: Pipeline<MemberElement>,

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
                Some((WebRtcPlayId(id.clone()), spec))
            }
            _ => None,
        })
    }

    pub fn get_publish_endpoint(
        &self,
        id: &WebRtcPublishId,
    ) -> Option<&WebRtcPublishEndpoint> {
        let e = self.pipeline.get(&id.0)?;
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
                Some((WebRtcPublishId(id.clone()), spec))
            }
            _ => None,
        })
    }

    pub fn credentials(&self) -> &str {
        &self.credentials
    }
}

fn generate_member_credentials() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(MEMBER_CREDENTIALS_LEN)
        .collect()
}

impl TryFrom<&MemberProto> for MemberSpec {
    type Error = TryFromProtobufError;

    /// Serialize [`MemberSpec`] from protobuf object.
    fn try_from(value: &MemberProto) -> Result<Self, Self::Error> {
        let mut pipeline = StdHashMap::new();
        for (id, member_element) in value.get_pipeline() {
            let endpoint = Endpoint::try_from(member_element)?;
            pipeline.insert(id.clone(), endpoint.into());
        }
        let pipeline = Pipeline::new(pipeline);

        let proto_credentials = value.get_credentials();
        let credentials = if proto_credentials.is_empty() {
            generate_member_credentials()
        } else {
            proto_credentials.to_string()
        };

        // Credentials here maybe absent.
        Ok(Self {
            pipeline,
            credentials,
        })
    }
}

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

//! Definitions and implementations of [Control API]'s `Member` element.
//!
//! [Control API]: http://tiny.cc/380uaz

use std::convert::TryFrom;

use derive_more::{Display, From};
use serde::Deserialize;

use super::{
    endpoint::{WebRtcPlayEndpoint, WebRtcPublishEndpoint},
    pipeline::Pipeline,
    room::RoomElement,
    TryFromElementError,
};

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

impl MemberSpec {
    /// Returns all [`WebRtcPlayEndpoint`]s of this [`MemberSpec`].
    pub fn play_endpoints(
        &self,
    ) -> impl Iterator<Item = (&String, &WebRtcPlayEndpoint)> {
        self.pipeline.iter().filter_map(|(id, e)| match e {
            MemberElement::WebRtcPlayEndpoint { spec } => Some((id, spec)),
            _ => None,
        })
    }

    /// Lookup [`WebRtcPublishEndpoint`] by ID.
    pub fn get_publish_endpoint_by_id(
        &self,
        id: &str,
    ) -> Option<&WebRtcPublishEndpoint> {
        let e = self.pipeline.get(id)?;
        if let MemberElement::WebRtcPublishEndpoint { spec } = e {
            Some(spec)
        } else {
            None
        }
    }

    /// Returns all [`WebRtcPublishEndpoint`]s of this [`MemberSpec`].
    pub fn publish_endpoints(
        &self,
    ) -> impl Iterator<Item = (&String, &WebRtcPublishEndpoint)> {
        self.pipeline.iter().filter_map(|(id, e)| match e {
            MemberElement::WebRtcPublishEndpoint { spec } => Some((id, spec)),
            _ => None,
        })
    }

    /// Returns credentials from this [`MemberSpec`].
    pub fn credentials(&self) -> &str {
        &self.credentials
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

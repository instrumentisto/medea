//! Member definitions and implementations.

use std::convert::TryFrom;

use hashbrown::HashMap;
use macro_attr::*;
use newtype_derive::{newtype_fmt, NewtypeDisplay, NewtypeFrom};
use serde::Deserialize;

use super::{
    endpoint::{SerdeWebRtcPlayEndpoint, SerdeWebRtcPublishEndpoint},
    pipeline::Pipeline,
    Element, TryFromElementError,
};

use crate::api::control::model::endpoint::webrtc::WebRtcPublishId;
pub use crate::api::control::model::member::Id;

/// Newtype for [`Element::Member`] variant.
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug)]
pub struct MemberSpec {
    /// Spec of this `Member`.
    pipeline: Pipeline,

    /// Credentials to authorize `Member` with.
    credentials: String,
}

impl MemberSpec {
    /// Returns all [`WebRtcPlayEndpoint`]s of this [`MemberSpec`].
    pub fn play_endpoints(&self) -> HashMap<&String, &SerdeWebRtcPlayEndpoint> {
        self.pipeline
            .iter()
            .filter_map(|(id, e)| match e {
                Element::WebRtcPlayEndpoint { spec } => Some((id, spec)),
                _ => None,
            })
            .collect()
    }

    /// Returns all [`WebRtcPublishEndpoint`]s of this [`MemberSpec`].
    pub fn publish_endpoints(
        &self,
    ) -> HashMap<WebRtcPublishId, &SerdeWebRtcPublishEndpoint> {
        self.pipeline
            .iter()
            .filter_map(|(id, e)| match e {
                Element::WebRtcPublishEndpoint { spec } => {
                    Some((WebRtcPublishId(id.clone()), spec))
                }
                _ => None,
            })
            .collect()
    }

    pub fn credentials(&self) -> &str {
        &self.credentials
    }
}

impl TryFrom<&Element> for MemberSpec {
    type Error = TryFromElementError;

    fn try_from(from: &Element) -> Result<Self, Self::Error> {
        match from {
            Element::Member { spec, credentials } => Ok(Self {
                pipeline: spec.clone(),
                credentials: credentials.clone(),
            }),
            _ => Err(TryFromElementError::NotMember),
        }
    }
}

//! Member definitions and implementations.

use std::{convert::TryFrom, fmt::Display, sync::Arc};

use serde::Deserialize;

use super::{pipeline::Pipeline, Element, TryFromElementError};

use crate::api::control::endpoint::{
    WebRtcPlayEndpoint, WebRtcPublishEndpoint,
};

/// ID of [`Member`].
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq)]
pub struct Id(pub String);

impl Display for Id {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(fmt, "{}", self.0)
    }
}

/// Media server user with its ID, credentials and spec.
#[derive(Clone, Debug)]
pub struct Member {
    /// ID of [`Member`].
    pub id: Id,

    /// Control API specification of this [`Member`].
    pub spec: Arc<MemberSpec>,

    /// Receivers of this [`Member`]'s publish endpoints.
    pub receivers: Vec<Id>,
}

/// Newtype for [`Element::Member`] variant.
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug)]
pub struct MemberSpec {
    /// Spec of this [`Member`].
    pub spec: Pipeline,

    /// Credentials to authorize [`Member`] with.
    pub credentials: String,
}

impl MemberSpec {
    /// Get all [`WebRtcPlayEndpoint`]s of this [`MemberSpec`].
    pub fn play_endpoints(&self) -> Vec<&WebRtcPlayEndpoint> {
        self.spec
            .pipeline
            .iter()
            .filter_map(|(_, e)| match e {
                Element::WebRtcPlayEndpoint { spec } => Some(spec),
                _ => None,
            })
            .collect()
    }

    /// Get all [`WebRtcPublishEndpoint`]s of this [`MemberSpec`].
    pub fn publish_endpoints(&self) -> Vec<&WebRtcPublishEndpoint> {
        self.spec
            .pipeline
            .iter()
            .filter_map(|(_, e)| match e {
                Element::WebRtcPublishEndpoint { spec } => Some(spec),
                _ => None,
            })
            .collect()
    }
}

impl TryFrom<Element> for MemberSpec {
    type Error = TryFromElementError;

    fn try_from(from: Element) -> Result<Self, Self::Error> {
        match from {
            Element::Member { spec, credentials } => {
                Ok(Self { spec, credentials })
            }
            _ => Err(TryFromElementError::NotMember),
        }
    }
}

//! Member definitions and implementations.

use std::{convert::TryFrom, fmt::Display, sync::Arc};

use serde::Deserialize;

use super::{element::Element, pipeline::Pipeline, Entity, TryFromEntityError};

use crate::api::control::element::{WebRtcPlayEndpoint, WebRtcPublishEndpoint};

/// ID of [`Member`].
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq)]
pub struct Id(pub String);

impl Display for Id {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(fmt, "{}", self.0)
    }
}

/// Media server user with its ID and credentials.
#[derive(Debug, Clone)]
pub struct Member {
    /// ID of [`Member`].
    pub id: Id,

    /// Control API specification of this [`Member`].
    pub spec: Arc<MemberSpec>,
}

/// Newtype for [`Entity::Member`] variant.
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug)]
pub struct MemberSpec {
    /// Spec of this [`Member`].
    pub spec: Pipeline,

    /// Credentials to authorize [`Member`] with.
    pub credentials: String,
}

impl MemberSpec {
    /// Get [`Element`] of this [`MemberSpec`] by ID.
    pub fn get_element(
        &self,
        id: &str,
    ) -> Option<Result<Element, TryFromEntityError>> {
        Some(Element::try_from(self.spec.pipeline.get(id).cloned()?))
    }

    /// Get all [`WebRtcPlayEndpoint`]s of this [`MemberSpec`].
    pub fn get_play_endpoints(&self) -> Vec<&WebRtcPlayEndpoint> {
        self.spec
            .pipeline
            .iter()
            .filter_map(|(_, e)| match e {
                Entity::WebRtcPlayEndpoint { spec } => Some(spec),
                _ => None,
            })
            .collect()
    }

    /// Get all [`WebRtcPublishEndpoint`]s of this [`MemberSpec`].
    pub fn get_publish_endpoints(&self) -> Vec<&WebRtcPublishEndpoint> {
        self.spec
            .pipeline
            .iter()
            .filter_map(|(_, e)| match e {
                Entity::WebRtcPublishEndpoint { spec } => Some(spec),
                _ => None,
            })
            .collect()
    }
}

impl TryFrom<Entity> for MemberSpec {
    type Error = TryFromEntityError;

    fn try_from(from: Entity) -> Result<Self, Self::Error> {
        match from {
            Entity::Member { spec, credentials } => {
                Ok(Self { spec, credentials })
            }
            _ => Err(TryFromEntityError::NotMember),
        }
    }
}

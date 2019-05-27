//! Member definitions and implementations.

use std::{convert::TryFrom, sync::Arc};

use super::{element::Element, pipeline::Pipeline, Entity, TryFromEntityError};

use crate::api::control::element::{WebRtcPlayEndpoint, WebRtcPublishEndpoint};

pub type Id = u64;

/// Media server user with its ID and credentials.
#[derive(Debug, Clone)]
pub struct Member {
    /// ID of [`Member`].
    pub id: Id,

    /// Control API specification of this [`Member`].
    pub spec: Arc<MemberSpec>,

    /// ID from Control API specification of this [`Member`].
    pub control_id: String,
}

/// Newtype for [`Entity::Member`] variant.
#[derive(Clone, Debug)]
#[allow(clippy::module_name_repetitions)]
pub struct MemberSpec {
    pub pipeline: Pipeline,
    pub credentials: String,
}

impl MemberSpec {
    /// Get [`Element`] of this [`MemberSpec`] by ID.
    pub fn get_element(
        &self,
        id: &str,
    ) -> Option<Result<Element, TryFromEntityError>> {
        Some(Element::try_from(self.pipeline.pipeline.get(id).cloned()?))
    }

    /// Get all [`WebRtcPlayEndpoint`]s of this [`MemberSpec`].
    pub fn get_play_endpoints(&self) -> Vec<&WebRtcPlayEndpoint> {
        self.pipeline
            .pipeline
            .iter()
            .filter_map(|(_name, e)| match e {
                Entity::WebRtcPlayEndpoint { spec } => Some(spec),
                _ => None,
            })
            .collect()
    }

    /// Get all [`WebRtcPlayEndpoint`]s by ID of [`MemberSpec`].
    pub fn get_play_endpoint_by_member_id(
        &self,
        src: &str,
    ) -> Vec<&WebRtcPlayEndpoint> {
        self.get_play_endpoints()
            .into_iter()
            .filter(|endpoint| endpoint.src.member_id == src)
            .collect()
    }

    /// Get all [`WebRtcPublishEndpoint`]s of this [`MemberSpec`].
    pub fn get_publish_endpoints(&self) -> Vec<&WebRtcPublishEndpoint> {
        self.pipeline
            .pipeline
            .iter()
            .filter_map(|(_name, e)| match e {
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
            Entity::Member { spec, credentials } => Ok(Self {
                pipeline: spec,
                credentials,
            }),
            _ => Err(TryFromEntityError::NotMember),
        }
    }
}

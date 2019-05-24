//! Member definitions and implementations.

use std::convert::TryFrom;

use super::{element::Element, pipeline::Pipeline, Entity, TryFromEntityError};
use crate::api::control::element::{WebRtcPlayEndpoint, WebRtcPublishEndpoint};

pub type Id = u64;

/// Media server user with its ID and credentials.
#[derive(Debug, Clone)]
pub struct Member {
    /// ID of [`Member`].
    pub id: Id,

    /// Credentials to authorize [`Member`] with.
    pub credentials: String,

    pub spec: MemberSpec,

    pub control_id: String,
}

#[derive(Clone, Debug)]
pub struct MemberSpec(pub Pipeline);
impl MemberSpec {
    pub fn get_element(
        &self,
        id: &str,
    ) -> Option<Result<Element, TryFromEntityError>> {
        Some(Element::try_from(self.0.pipeline.get(id).cloned()?))
    }

    pub fn get_play_endpoints(&self) -> Vec<WebRtcPlayEndpoint> {
        self.0
            .pipeline
            .iter()
            .filter_map(|(_name, e)| match e {
                Entity::WebRtcPlayEndpoint { spec } => Some(spec),
                _ => None,
            })
            .cloned()
            .collect()
    }

    pub fn get_play_endpoint_by_src(
        &self,
        src: &str,
    ) -> Vec<WebRtcPlayEndpoint> {
        self.get_play_endpoints()
            .iter()
            .filter(|endpoint| endpoint.src.member_id == src)
            .cloned()
            .collect()
    }

    pub fn get_publish_endpoints(&self) -> Vec<WebRtcPublishEndpoint> {
        self.0
            .pipeline
            .iter()
            .filter_map(|(_name, e)| match e {
                Entity::WebRtcPublishEndpoint { spec } => Some(spec),
                _ => None,
            })
            .cloned()
            .collect()
    }
}

impl TryFrom<Entity> for MemberSpec {
    type Error = TryFromEntityError;

    fn try_from(from: Entity) -> Result<Self, Self::Error> {
        match from {
            Entity::Member { spec } => Ok(MemberSpec(spec)),
            _ => Err(TryFromEntityError::NotMember),
        }
    }
}

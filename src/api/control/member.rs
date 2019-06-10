//! Member definitions and implementations.

use std::{convert::TryFrom, fmt::Display, sync::Arc};

use hashbrown::HashMap;
use serde::Deserialize;

use super::{
    endpoint::{WebRtcPlayEndpoint, WebRtcPublishEndpoint},
    pipeline::Pipeline,
    Element, TryFromElementError,
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
    id: Id,

    /// Control API specification of this [`Member`].
    spec: Arc<MemberSpec>,

    /// Pipeline of [`Room`] in which this [`Member`] is located.
    room_pipeline: Arc<Pipeline>,
}

impl Member {
    pub fn new(id: Id, spec: MemberSpec, room_pipeline: Arc<Pipeline>) -> Self {
        Self {
            spec: Arc::new(spec),
            id,
            room_pipeline,
        }
    }

    /// Returns [`Id`] of [`Member`].
    pub fn id(&self) -> &Id {
        &self.id
    }

    /// Returns credentials to authorize [`Member`] with.
    pub fn credentials(&self) -> &str {
        self.spec.credentials()
    }

    /// Returns all [`WebRtcPlayEndpoint`]s of this [`Member`].
    pub fn play_endpoints(&self) -> HashMap<&String, &WebRtcPlayEndpoint> {
        self.spec.play_endpoints()
    }

    /// Returns all [`WebRtcPublishEndpoint`]s of this [`Member`].
    pub fn publish_endpoints(
        &self,
    ) -> HashMap<&String, &WebRtcPublishEndpoint> {
        self.spec.publish_endpoints()
    }

    // TODO: remove this func in favor of extending WebRtcPublishEndpoint with
    //       list of play endpoints, and play endpoint should have member_id (or maybe
    //       ref to parent element?)

    /// Get all receivers [`Id`] of all [`Member`]'s [`WebRtcPublishEndpoint`]s.
    ///
    /// Returns [`TryFromElementError::NotMember`] when not member finded in
    /// [`RoomSpec`]'s [`Pipeline`].
    #[allow(clippy::block_in_if_condition_stmt)]
    pub fn receivers(&self) -> Result<Vec<Id>, TryFromElementError> {
        let mut members = HashMap::new();
        for (id, element) in self.room_pipeline.iter() {
            members.insert(id, MemberSpec::try_from(element)?);
        }

        Ok(members
            .into_iter()
            .filter_map(|(id, member)| {
                if member
                    .play_endpoints()
                    .iter()
                    .filter(|(_, e)| e.src.member_id == self.id)
                    .filter(|(_, e)| {
                        self.spec
                            .publish_endpoints()
                            .contains_key(&e.src.endpoint_id)
                    })
                    .count()
                    > 0
                {
                    Some(Id(id.clone()))
                } else {
                    None
                }
            })
            .collect())
    }
}

/// Newtype for [`Element::Member`] variant.
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug)]
pub struct MemberSpec {
    /// Spec of this [`Member`].
    pipeline: Pipeline,

    /// Credentials to authorize [`Member`] with.
    credentials: String,
}

impl MemberSpec {
    /// Returns all [`WebRtcPlayEndpoint`]s of this [`MemberSpec`].
    pub fn play_endpoints(&self) -> HashMap<&String, &WebRtcPlayEndpoint> {
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
    ) -> HashMap<&String, &WebRtcPublishEndpoint> {
        self.pipeline
            .iter()
            .filter_map(|(id, e)| match e {
                Element::WebRtcPublishEndpoint { spec } => Some((id, spec)),
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

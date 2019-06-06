//! Member definitions and implementations.

use std::{convert::TryFrom, fmt::Display, sync::Arc};

use serde::Deserialize;

use super::{
    pipeline::Pipeline,
    room::RoomSpec,
    Element,
    TryFromElementError,
    endpoint::{
        WebRtcPublishEndpoint,
        WebRtcPlayEndpoint,
    }
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

    /// Receivers of this [`Member`]'s publish endpoints.
    receivers: Vec<Id>,
}

impl Member {
    pub fn new(
        id: Id,
        spec: MemberSpec,
        room_spec: &RoomSpec,
    ) -> Result<Self, TryFromElementError> {
        Ok(Self {
            receivers: room_spec.get_receivers_for_member(&id)?,
            spec: Arc::new(spec),
            id,
        })
    }

    /// Returns [`Id`] of [`Member`].
    pub fn id(&self) -> &Id {
        &self.id
    }

    /// Returns credentials to authorize [`Member`] with.
    pub fn credentials(&self) -> &String {
        &self.spec.credentials
    }

    /// Returns all [`WebRtcPlayEndpoint`]s of this [`Member`].
    pub fn play_endpoints(&self) -> Vec<&WebRtcPlayEndpoint> {
        self.spec.play_endpoints()
    }

    /// Returns all [`WebRtcPublishEndpoint`]s of this [`Member`].
    pub fn publish_endpoints(&self) -> Vec<&WebRtcPublishEndpoint> {
        self.spec.publish_endpoints()
    }

    /// Returns all receivers [`Id`] of this [`Member`].
    pub fn receivers(&self) -> &Vec<Id> {
        &self.receivers
    }
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

//! Member definitions and implementations.

use std::convert::TryFrom;

use hashbrown::HashMap;
use macro_attr::*;
use newtype_derive::{newtype_fmt, NewtypeDisplay, NewtypeFrom};
use serde::Deserialize;

use super::{
    endpoint::{WebRtcPlayEndpoint, WebRtcPublishEndpoint},
    pipeline::Pipeline,
    Element, TryFromElementError,
};

macro_attr! {
    /// ID of `Member`.
    #[derive(
        Clone,
        Debug,
        Deserialize,
        Eq,
        Hash,
        PartialEq,
        NewtypeFrom!,
        NewtypeDisplay!
    )]
    pub struct Id(pub String);
}

/// Newtype for [`Element::Member`] variant.
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug)]
pub struct MemberSpec {
    /// Spec of this `Member`.
    pipeline: Pipeline<Element>,

    /// Credentials to authorize `Member` with.
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

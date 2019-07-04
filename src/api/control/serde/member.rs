//! Member definitions and implementations.

use std::convert::TryFrom;

use hashbrown::HashMap;

use crate::api::control::{
    model::{
        endpoint::webrtc::{
            WebRtcPlayEndpoint, WebRtcPlayId, WebRtcPublishEndpoint,
            WebRtcPublishId,
        },
        member::MemberSpec,
    },
    serde::SerdeEndpoint,
};

use super::{
    endpoint::{SerdeWebRtcPlayEndpointImpl, SerdeWebRtcPublishEndpointImpl},
    pipeline::Pipeline,
    Element, TryFromElementError,
};

/// Newtype for [`Element::Member`] variant.
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug)]
pub struct SerdeMemberSpecImpl {
    /// Spec of this `Member`.
    pipeline: Pipeline,

    /// Credentials to authorize `Member` with.
    credentials: String,
}

impl SerdeMemberSpecImpl {
    /// Returns all [`WebRtcPlayEndpoint`]s of this [`MemberSpec`].
    pub fn play_endpoints(
        &self,
    ) -> HashMap<&String, &SerdeWebRtcPlayEndpointImpl> {
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
    ) -> HashMap<WebRtcPublishId, &SerdeWebRtcPublishEndpointImpl> {
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

impl MemberSpec for SerdeMemberSpecImpl {
    fn webrtc_play_endpoints(
        &self,
    ) -> HashMap<WebRtcPlayId, Box<dyn WebRtcPlayEndpoint>> {
        self.pipeline
            .iter()
            .filter_map(|(id, e)| match e {
                Element::WebRtcPlayEndpoint { spec } => Some((
                    WebRtcPlayId(id.clone()),
                    Box::new(spec.clone()) as Box<dyn WebRtcPlayEndpoint>,
                )),
                _ => None,
            })
            .collect()
    }

    fn webrtc_publish_endpoints(
        &self,
    ) -> HashMap<WebRtcPublishId, Box<dyn WebRtcPublishEndpoint>> {
        self.pipeline
            .iter()
            .filter_map(|(id, e)| match e {
                Element::WebRtcPublishEndpoint { spec } => Some((
                    WebRtcPublishId(id.clone()),
                    Box::new(spec.clone()) as Box<dyn WebRtcPublishEndpoint>,
                )),
                _ => None,
            })
            .collect()
    }

    fn credentials(&self) -> &str {
        &self.credentials
    }

    fn get_webrtc_play_by_id(
        &self,
        id: &WebRtcPlayId,
    ) -> Option<Box<dyn WebRtcPlayEndpoint>> {
        let element = self.pipeline.get(&id.0)?;

        if let Some(endpoint) = SerdeEndpoint::try_from(element).ok() {
            if let SerdeEndpoint::WebRtcPlay(e) = endpoint {
                return Some(Box::new(e) as Box<dyn WebRtcPlayEndpoint>);
            }
        }
        None
    }

    fn get_webrtc_publish_by_id(
        &self,
        id: &WebRtcPublishId,
    ) -> Option<Box<dyn WebRtcPublishEndpoint>> {
        let element = self.pipeline.get(&id.0)?;
        if let Some(endpoint) = SerdeEndpoint::try_from(element).ok() {
            if let SerdeEndpoint::WebRtcPublish(e) = endpoint {
                return Some(Box::new(e) as Box<dyn WebRtcPublishEndpoint>);
            }
        }
        None
    }
}

impl TryFrom<&Element> for SerdeMemberSpecImpl {
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

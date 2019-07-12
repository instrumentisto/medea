//! Endpoint elements of medea spec.

pub mod webrtc_play_endpoint;
pub mod webrtc_publish_endpoint;

use std::convert::TryFrom;

use crate::api::control::grpc::protos::control::{
    CreateRequest, Member_Element as MemberElementProto,
};

use super::{Element, TryFromElementError, TryFromProtobufError};

#[doc(inline)]
pub use webrtc_play_endpoint::WebRtcPlayEndpoint;
#[doc(inline)]
pub use webrtc_publish_endpoint::WebRtcPublishEndpoint;

/// [`Endpoint`] represents a media element that one or more media data streams
/// flow through.
#[derive(Debug)]
pub enum Endpoint {
    WebRtcPublish(WebRtcPublishEndpoint),
    WebRtcPlay(WebRtcPlayEndpoint),
}

impl Into<Element> for Endpoint {
    fn into(self) -> Element {
        match self {
            Endpoint::WebRtcPublish(e) => {
                Element::WebRtcPublishEndpoint { spec: e }
            }
            Endpoint::WebRtcPlay(e) => Element::WebRtcPlayEndpoint { spec: e },
        }
    }
}

impl TryFrom<&MemberElementProto> for Endpoint {
    type Error = TryFromProtobufError;

    fn try_from(value: &MemberElementProto) -> Result<Self, Self::Error> {
        if value.has_webrtc_play() {
            let play = WebRtcPlayEndpoint::try_from(value.get_webrtc_play())?;
            Ok(Endpoint::WebRtcPlay(play))
        } else if value.has_webrtc_pub() {
            let publish = WebRtcPublishEndpoint::from(value.get_webrtc_pub());
            Ok(Endpoint::WebRtcPublish(publish))
        } else {
            // TODO implement another endpoints when they will be implemented
            unimplemented!()
        }
    }
}

impl TryFrom<&CreateRequest> for Endpoint {
    type Error = TryFromProtobufError;

    fn try_from(value: &CreateRequest) -> Result<Self, Self::Error> {
        if value.has_webrtc_play() {
            let play = WebRtcPlayEndpoint::try_from(value.get_webrtc_play())?;
            Ok(Endpoint::WebRtcPlay(play))
        } else if value.has_webrtc_pub() {
            let publish = WebRtcPublishEndpoint::from(value.get_webrtc_pub());
            Ok(Endpoint::WebRtcPublish(publish))
        } else {
            // TODO implement another endpoints when they will be implemented
            unimplemented!()
        }
    }
}

impl TryFrom<&Element> for Endpoint {
    type Error = TryFromElementError;

    fn try_from(from: &Element) -> Result<Self, Self::Error> {
        match from {
            Element::WebRtcPlayEndpoint { spec } => {
                Ok(Endpoint::WebRtcPlay(spec.clone()))
            }
            Element::WebRtcPublishEndpoint { spec } => {
                Ok(Endpoint::WebRtcPublish(spec.clone()))
            }
            _ => Err(TryFromElementError::NotEndpoint),
        }
    }
}

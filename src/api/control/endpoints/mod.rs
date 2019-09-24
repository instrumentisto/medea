//! Endpoint elements of [medea] spec.
//!
//! [medea]: https://github.com/instrumentisto/medea

pub mod webrtc_play_endpoint;
pub mod webrtc_publish_endpoint;

use std::convert::TryFrom;

use derive_more::{Display, From, Into};
use serde::Deserialize;

use medea_control_api_proto::grpc::control_api::{
    CreateRequest_oneof_el as ElementProto,
    Member_Element_oneof_el as MemberElementProto,
};

use super::{member::MemberElement, TryFromProtobufError};

#[doc(inline)]
pub use webrtc_play_endpoint::{WebRtcPlayEndpoint, WebRtcPlayId};
#[doc(inline)]
pub use webrtc_publish_endpoint::{WebRtcPublishEndpoint, WebRtcPublishId};

/// ID of `Endpoint`.
#[derive(
    Clone, Debug, Eq, Hash, Deserialize, PartialEq, From, Display, Into,
)]
pub struct Id(pub String);

macro_rules! impl_from_into {
    ($id:ty) => {
        impl std::convert::From<Id> for $id {
            fn from(id: Id) -> Self {
                String::from(id).into()
            }
        }

        impl std::convert::From<$id> for Id {
            fn from(id: $id) -> Self {
                String::from(id).into()
            }
        }
    };
}

impl_from_into!(WebRtcPublishId);
impl_from_into!(WebRtcPlayId);

/// Media element that one or more media data streams flow through.
#[derive(Debug, From)]
pub enum EndpointSpec {
    // TODO: add id in endpoints
    /// [`WebRtcPublishEndpoint`] element.
    WebRtcPublish(WebRtcPublishEndpoint),

    /// [`WebRtcPlayEndpoint`] element.
    WebRtcPlay(WebRtcPlayEndpoint),
}

impl Into<MemberElement> for EndpointSpec {
    fn into(self) -> MemberElement {
        match self {
            Self::WebRtcPublish(e) => {
                MemberElement::WebRtcPublishEndpoint { spec: e }
            }
            Self::WebRtcPlay(e) => {
                MemberElement::WebRtcPlayEndpoint { spec: e }
            }
        }
    }
}

impl TryFrom<(Id, MemberElementProto)> for EndpointSpec {
    type Error = TryFromProtobufError;

    fn try_from(value: (Id, MemberElementProto)) -> Result<Self, Self::Error> {
        use MemberElementProto::*;

        let id = value.0;
        let proto = value.1;

        match proto {
            webrtc_play(elem) => {
                let play = WebRtcPlayEndpoint::try_from(&elem)?;
                Ok(Self::WebRtcPlay(play))
            }
            webrtc_pub(elem) => {
                let publish = WebRtcPublishEndpoint::from(&elem);
                Ok(Self::WebRtcPublish(publish))
            }
            hub(_) | file_recorder(_) | relay(_) => {
                Err(TryFromProtobufError::UnimplementedEndpoint(id.0))
            }
        }
    }
}

impl TryFrom<(Id, ElementProto)> for EndpointSpec {
    type Error = TryFromProtobufError;

    fn try_from((id, proto): (Id, ElementProto)) -> Result<Self, Self::Error> {
        use ElementProto::*;

        match proto {
            webrtc_play(elem) => {
                let play = WebRtcPlayEndpoint::try_from(&elem)?;
                Ok(Self::WebRtcPlay(play))
            }
            webrtc_pub(elem) => {
                let publish = WebRtcPublishEndpoint::from(&elem);
                Ok(Self::WebRtcPublish(publish))
            }
            hub(_) | file_recorder(_) | relay(_) => {
                Err(TryFromProtobufError::UnimplementedEndpoint(id.0))
            }
            member(_) | room(_) => {
                Err(TryFromProtobufError::ExpectedOtherElement(
                    String::from("Endpoint"),
                    id.0,
                ))
            }
        }
    }
}

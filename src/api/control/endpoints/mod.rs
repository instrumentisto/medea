//! Endpoint elements of [Medea] spec.
//!
//! [Medea]: https://github.com/instrumentisto/medea

pub mod webrtc_play_endpoint;
pub mod webrtc_publish_endpoint;

use std::convert::TryFrom;

use derive_more::{Display, From};
use medea_control_api_proto::grpc::api as proto;
use serde::Deserialize;

use super::{member::MemberElement, TryFromProtobufError};

#[doc(inline)]
pub use webrtc_play_endpoint::{WebRtcPlayEndpoint, WebRtcPlayId};
#[doc(inline)]
pub use webrtc_publish_endpoint::{WebRtcPublishEndpoint, WebRtcPublishId};

/// ID of `Endpoint`.
#[derive(Clone, Debug, Deserialize, Display, Eq, From, Hash, PartialEq)]
#[from(forward)]
pub struct Id(pub String);

macro_rules! impl_from_into {
    ($id:ty) => {
        impl std::convert::From<Id> for $id {
            fn from(id: Id) -> Self {
                <$id>::from(id.0)
            }
        }

        impl std::convert::From<$id> for Id {
            fn from(id: $id) -> Self {
                Id::from(id.0)
            }
        }
    };
}

impl_from_into!(WebRtcPublishId);
impl_from_into!(WebRtcPlayId);

/// Media element that one or more media data streams flow through.
#[derive(Debug, From)]
pub enum EndpointSpec {
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

impl TryFrom<(Id, proto::member::element::El)> for EndpointSpec {
    type Error = TryFromProtobufError;

    fn try_from(
        (_, proto): (Id, proto::member::element::El),
    ) -> Result<Self, Self::Error> {
        use proto::member::element::El;

        match proto {
            El::WebrtcPlay(elem) => {
                let play = WebRtcPlayEndpoint::try_from(&elem)?;
                Ok(Self::WebRtcPlay(play))
            }
            El::WebrtcPub(elem) => {
                let publish = WebRtcPublishEndpoint::try_from(&elem)?;
                Ok(Self::WebRtcPublish(publish))
            }
        }
    }
}

impl TryFrom<(Id, proto::create_request::El)> for EndpointSpec {
    type Error = TryFromProtobufError;

    fn try_from(
        (id, proto): (Id, proto::create_request::El),
    ) -> Result<Self, Self::Error> {
        use proto::create_request::El;

        match proto {
            El::WebrtcPlay(elem) => {
                let play = WebRtcPlayEndpoint::try_from(&elem)?;
                Ok(Self::WebRtcPlay(play))
            }
            El::WebrtcPub(elem) => {
                let publish = WebRtcPublishEndpoint::try_from(&elem)?;
                Ok(Self::WebRtcPublish(publish))
            }
            El::Member(_) | El::Room(_) => {
                Err(TryFromProtobufError::ExpectedOtherElement(
                    String::from("Endpoint"),
                    id.0,
                ))
            }
        }
    }
}

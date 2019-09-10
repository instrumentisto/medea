//! Endpoint elements of medea spec.

pub mod webrtc_play_endpoint;
pub mod webrtc_publish_endpoint;

use std::convert::TryFrom;

use medea_grpc_proto::control::{
    CreateRequest, Member_Element as MemberElementProto,
};

use super::{member::MemberElement, TryFromProtobufError};

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

impl Into<MemberElement> for Endpoint {
    fn into(self) -> MemberElement {
        match self {
            Endpoint::WebRtcPublish(e) => {
                MemberElement::WebRtcPublishEndpoint { spec: e }
            }
            Endpoint::WebRtcPlay(e) => {
                MemberElement::WebRtcPlayEndpoint { spec: e }
            }
        }
    }
}

macro_rules! impl_try_from_proto_for_endpoint {
    ($proto:ty) => {
        impl TryFrom<$proto> for Endpoint {
            type Error = TryFromProtobufError;

            fn try_from(proto: $proto) -> Result<Self, Self::Error> {
                if proto.has_webrtc_play() {
                    let play =
                        WebRtcPlayEndpoint::try_from(proto.get_webrtc_play())?;
                    Ok(Endpoint::WebRtcPlay(play))
                } else if proto.has_webrtc_pub() {
                    let publish =
                        WebRtcPublishEndpoint::from(proto.get_webrtc_pub());
                    Ok(Endpoint::WebRtcPublish(publish))
                } else {
                    // TODO implement another endpoints when they will be
                    // implemented
                    unimplemented!()
                }
            }
        }
    };
}

impl_try_from_proto_for_endpoint!(&MemberElementProto);
impl_try_from_proto_for_endpoint!(&CreateRequest);

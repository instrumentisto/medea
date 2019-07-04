use crate::{
    api::{
        control::{
            model::{
                endpoint::webrtc::{WebRtcPlayEndpoint, WebRtcPublishEndpoint},
                member::MemberSpec,
                MemberId, RoomId,
            },
            serde::endpoint::{P2pMode, SerdeSrcUri},
        },
        grpc::protos::control::WebRtcPublishEndpoint as WebRtcPublishEndpointDto,
    },
    signalling::elements::endpoints::webrtc::{WebRtcPlayId, WebRtcPublishId},
};
use hashbrown::HashMap;

pub struct GrpcWebRtcPublishEndpoint(pub WebRtcPublishEndpointDto);

impl WebRtcPublishEndpoint for GrpcWebRtcPublishEndpoint {
    fn p2p(&self) -> P2pMode {
        // TODO: implement me.
        P2pMode::Always
    }
}

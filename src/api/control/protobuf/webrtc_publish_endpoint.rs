use crate::api::{
    control::{
        model::endpoint::webrtc::WebRtcPublishEndpoint,
        serde::endpoint::P2pMode,
    },
    grpc::protos::control::{
        WebRtcPublishEndpoint as WebRtcPublishEndpointProto,
        WebRtcPublishEndpoint_P2P,
    },
};

pub struct GrpcWebRtcPublishEndpointSpecImpl(pub WebRtcPublishEndpointProto);

impl WebRtcPublishEndpoint for GrpcWebRtcPublishEndpointSpecImpl {
    fn p2p(&self) -> P2pMode {
        if self.0.has_p2p() {
            let p2p = self.0.get_p2p();
            match p2p {
                WebRtcPublishEndpoint_P2P::ALWAYS => P2pMode::Always,
                WebRtcPublishEndpoint_P2P::NEVER => P2pMode::Never,
                WebRtcPublishEndpoint_P2P::IF_POSSIBLE => P2pMode::IfPossible,
            }
        } else {
            // TODO: do with me something
            unimplemented!()
        }
    }
}

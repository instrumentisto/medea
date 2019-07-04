use crate::api::{
    control::model::endpoint::webrtc::{P2pMode, WebRtcPublishEndpoint},
    grpc::protos::control::WebRtcPublishEndpoint as WebRtcPublishEndpointProto,
};

pub struct GrpcWebRtcPublishEndpointSpecImpl(pub WebRtcPublishEndpointProto);

impl WebRtcPublishEndpoint for GrpcWebRtcPublishEndpointSpecImpl {
    fn p2p(&self) -> P2pMode {
        if self.0.has_p2p() {
            let p2p = self.0.get_p2p();
            P2pMode::from(p2p)
        } else {
            // TODO: do with me something
            unimplemented!()
        }
    }
}

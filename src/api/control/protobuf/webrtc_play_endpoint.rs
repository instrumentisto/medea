use crate::{
    api::{
        control::{
            model::{endpoint::webrtc::WebRtcPlayEndpoint, MemberId, RoomId},
            serde::endpoint::SerdeSrcUri,
        },
        grpc::protos::control::WebRtcPlayEndpoint as WebRtcPlayEndpointDto,
    },
    signalling::elements::endpoints::webrtc::WebRtcPublishId,
};

pub struct GrpcWebRtcPlayEndpoint(pub WebRtcPlayEndpointDto);

impl WebRtcPlayEndpoint for GrpcWebRtcPlayEndpoint {
    fn src(&self) -> SerdeSrcUri {
        if self.0.has_src() {
            let _src = self.0.get_src();
            SerdeSrcUri {
                endpoint_id: WebRtcPublishId("".to_string()),
                member_id: MemberId("".to_string()),
                room_id: RoomId("".to_string()),
            }
        } else {
            // TODO: do something with it.
            unimplemented!("TODO")
        }
    }
}

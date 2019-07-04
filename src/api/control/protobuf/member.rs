use crate::{
    api::{
        control::{
            model::{
                endpoint::webrtc::{WebRtcPlayEndpoint, WebRtcPublishEndpoint},
                member::MemberSpec,
                MemberId, RoomId,
            },
            protobuf::{
                webrtc_play_endpoint::GrpcWebRtcPlayEndpoint,
                webrtc_publish_endpoint::GrpcWebRtcPublishEndpoint,
            },
        },
        grpc::protos::control::Member,
    },
    signalling::elements::endpoints::webrtc::{WebRtcPlayId, WebRtcPublishId},
};
use hashbrown::HashMap;

pub struct GrpcMember(pub Member);

impl MemberSpec for GrpcMember {
    fn webrtc_play_endpoints(
        &self,
    ) -> HashMap<WebRtcPlayId, Box<dyn WebRtcPlayEndpoint>> {
        self.0
            .get_pipeline()
            .iter()
            .filter_map(|(id, element)| {
                if element.has_webrtc_play() {
                    let endpoint = element.get_webrtc_play().clone();
                    Some((
                        WebRtcPlayId(id.clone()),
                        Box::new(GrpcWebRtcPlayEndpoint(endpoint))
                            as Box<dyn WebRtcPlayEndpoint>,
                    ))
                } else {
                    None
                }
            })
            .collect()
    }

    fn webrtc_publish_endpoints(
        &self,
    ) -> HashMap<WebRtcPublishId, Box<dyn WebRtcPublishEndpoint>> {
        self.0
            .get_pipeline()
            .iter()
            .filter_map(|(id, element)| {
                if element.has_webrtc_pub() {
                    let endpoint = element.get_webrtc_pub().clone();
                    Some((
                        WebRtcPublishId(id.clone()),
                        Box::new(GrpcWebRtcPublishEndpoint(endpoint))
                            as Box<dyn WebRtcPublishEndpoint>,
                    ))
                } else {
                    None
                }
            })
            .collect()
    }

    fn credentials(&self) -> &String {
        // TODO: deal with it
        unimplemented!()
    }

    fn id(&self) -> &MemberId {
        unimplemented!()
    }

    fn get_webrtc_play_by_id(
        &self,
        id: &WebRtcPlayId,
    ) -> Option<Box<dyn WebRtcPlayEndpoint>> {
        let element = self.0.pipeline.get(&id.0)?;
        if element.has_webrtc_play() {
            let play = element.get_webrtc_play().clone();
            let play = GrpcWebRtcPlayEndpoint(play);
            Some(Box::new(play) as Box<dyn WebRtcPlayEndpoint>)
        } else {
            None
        }
    }

    fn get_webrtc_publish_by_id(
        &self,
        id: &WebRtcPublishId,
    ) -> Option<Box<dyn WebRtcPublishEndpoint>> {
        let element = self.0.pipeline.get(&id.0)?;
        if element.has_webrtc_pub() {
            let publish = element.get_webrtc_pub().clone();
            let play = GrpcWebRtcPublishEndpoint(publish);
            Some(Box::new(play) as Box<dyn WebRtcPublishEndpoint>)
        } else {
            None
        }
    }
}

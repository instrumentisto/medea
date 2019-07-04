use hashbrown::HashMap;

use crate::{
    api::{
        control::{
            model::{
                endpoint::webrtc::{WebRtcPlayEndpoint, WebRtcPublishEndpoint},
                member::MemberSpec,
            },
            protobuf::{
                webrtc_play_endpoint::GrpcWebRtcPlayEndpointSpecImpl,
                webrtc_publish_endpoint::GrpcWebRtcPublishEndpointSpecImpl,
            },
        },
        grpc::protos::control::Member as MemberProto,
    },
    signalling::elements::endpoints::webrtc::{WebRtcPlayId, WebRtcPublishId},
};

pub struct GrpcMemberSpecImpl(pub MemberProto);

impl MemberSpec for GrpcMemberSpecImpl {
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
                        Box::new(GrpcWebRtcPlayEndpointSpecImpl(endpoint))
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
                        Box::new(GrpcWebRtcPublishEndpointSpecImpl(endpoint))
                            as Box<dyn WebRtcPublishEndpoint>,
                    ))
                } else {
                    None
                }
            })
            .collect()
    }

    fn credentials(&self) -> &str {
        if self.0.has_credentials() {
            self.0.get_credentials()
        } else {
            // TODO: deal with it
            unimplemented!()
        }
    }

    fn get_webrtc_play_by_id(
        &self,
        id: &WebRtcPlayId,
    ) -> Option<Box<dyn WebRtcPlayEndpoint>> {
        let element = self.0.pipeline.get(&id.0)?;
        if element.has_webrtc_play() {
            let play = element.get_webrtc_play().clone();
            let play = GrpcWebRtcPlayEndpointSpecImpl(play);
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
            let play = GrpcWebRtcPublishEndpointSpecImpl(publish);
            Some(Box::new(play) as Box<dyn WebRtcPublishEndpoint>)
        } else {
            None
        }
    }
}

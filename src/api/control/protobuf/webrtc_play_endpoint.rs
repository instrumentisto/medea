use crate::{
    api::{
        control::{
            model::{endpoint::webrtc::WebRtcPlayEndpoint, MemberId, RoomId},
            serde::endpoint::SerdeSrcUri,
        },
        grpc::protos::control::WebRtcPlayEndpoint as WebRtcPlayEndpointProto,
    },
    signalling::elements::endpoints::webrtc::WebRtcPublishId,
};

pub struct GrpcWebRtcPlayEndpointSpecImpl(pub WebRtcPlayEndpointProto);

impl WebRtcPlayEndpoint for GrpcWebRtcPlayEndpointSpecImpl {
    fn src(&self) -> SerdeSrcUri {
        if self.0.has_src() {
            let src = self.0.get_src();
            parse_src_uri(src)
        } else {
            // TODO: do something with it.
            unimplemented!("TODO")
        }
    }
}

// TODO: use already done implementation from serde DTO
//       share this with serde deseralizer.
fn parse_src_uri(value: &str) -> SerdeSrcUri {
    let protocol_name: String = value.chars().take(8).collect();
    if protocol_name != "local://" {
        panic!()
    }

    let uri_body = value.chars().skip(8).collect::<String>();
    let mut uri_body_splitted: Vec<&str> = uri_body.rsplit('/').collect();
    let uri_body_splitted_len = uri_body_splitted.len();
    if uri_body_splitted_len != 3 {
        let _error_msg = if uri_body_splitted_len == 0 {
            "room_id, member_id, endpoint_id"
        } else if uri_body_splitted_len == 1 {
            "member_id, endpoint_id"
        } else if uri_body_splitted_len == 2 {
            "endpoint_id"
        } else {
            panic!()
        };
        panic!()
    }
    let room_id = uri_body_splitted.pop().unwrap().to_string();
    if room_id.is_empty() {
        panic!()
    }
    let member_id = uri_body_splitted.pop().unwrap().to_string();
    if member_id.is_empty() {
        panic!()
    }
    let endpoint_id = uri_body_splitted.pop().unwrap().to_string();
    if endpoint_id.is_empty() {
        panic!()
    }

    SerdeSrcUri {
        room_id: RoomId(room_id),
        member_id: MemberId(member_id),
        endpoint_id: WebRtcPublishId(endpoint_id),
    }
}

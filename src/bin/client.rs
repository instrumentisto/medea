#![allow(dead_code)]

use std::{collections::HashMap, sync::Arc};

use grpcio::{ChannelBuilder, EnvBuilder};
use medea::api::control::grpc::protos::{
    control::{
        CreateRequest, IdRequest, Member, Member_Element, Room, Room_Element,
        WebRtcPlayEndpoint, WebRtcPublishEndpoint, WebRtcPublishEndpoint_P2P,
    },
    control_grpc::ControlApiClient,
};
use protobuf::RepeatedField;

fn main() {
    let env = Arc::new(EnvBuilder::new().build());
    let ch = ChannelBuilder::new(env).connect("localhost:50051");
    let client = ControlApiClient::new(ch);

    create_room(&client);
    //    delete_room(&client);
    //    delete_endpoint(&client);
    //        delete_member(&client);
    //    create_member(&client);
    //    std::thread::sleep(Duration::from_secs(1));
    create_endpoint(&client);
    get_room(&client);
}

fn create_room(client: &ControlApiClient) {
    let mut req = CreateRequest::new();
    let mut room = Room::new();
    let mut publisher = Member::new();
    let mut responder = Member::new();
    let mut play_endpoint = WebRtcPlayEndpoint::new();
    let mut publish_endpoint = WebRtcPublishEndpoint::new();

    play_endpoint.set_src("local://grpc-test/publisher/publish".to_string());
    let mut play_endpoint_element = Member_Element::new();
    play_endpoint_element.set_webrtc_play(play_endpoint);
    let mut responder_pipeline = HashMap::new();
    responder_pipeline.insert("play".to_string(), play_endpoint_element);
    responder.set_pipeline(responder_pipeline);
    responder.set_credentials("test".to_string());

    publish_endpoint.set_p2p(WebRtcPublishEndpoint_P2P::ALWAYS);
    let mut publish_endpoint_element = Member_Element::new();
    publish_endpoint_element.set_webrtc_pub(publish_endpoint);
    let mut publisher_pipeline = HashMap::new();
    publisher_pipeline.insert("publish".to_string(), publish_endpoint_element);
    publisher.set_pipeline(publisher_pipeline);
    publisher.set_credentials("test".to_string());

    let mut publisher_member_element = Room_Element::new();
    publisher_member_element.set_member(publisher);
    let mut responder_member_element = Room_Element::new();
    responder_member_element.set_member(responder);
    let mut room_pipeline = HashMap::new();
    room_pipeline.insert("publisher".to_string(), publisher_member_element);
    room_pipeline.insert("responder".to_string(), responder_member_element);
    room.set_pipeline(room_pipeline);
    req.set_room(room);
    req.set_id("local://grpc-test".to_string());

    let reply = client.create(&req).expect("create room");
    println!("{:?}", reply);
}

fn create_member(client: &ControlApiClient) {
    let mut create_member_request = CreateRequest::new();
    let mut member = Member::new();
    let mut member_pipeline = HashMap::new();

    let mut play_endpoint = WebRtcPlayEndpoint::new();
    play_endpoint.set_src("local://grpc-test/publisher/publish".to_string());
    let mut member_element = Member_Element::new();
    member_element.set_webrtc_play(play_endpoint);
    member_pipeline.insert("play".to_string(), member_element);

    member.set_credentials("test".to_string());
    member.set_pipeline(member_pipeline);
    create_member_request.set_id("local://grpc-test/player".to_string());
    create_member_request.set_member(member);

    let reply = client
        .create(&create_member_request)
        .expect("create member");
    println!("{:?}", reply)
}

fn create_endpoint(client: &ControlApiClient) {
    let mut create_endpoint_request = CreateRequest::new();
    let mut endpoint = WebRtcPublishEndpoint::new();
    endpoint.set_p2p(WebRtcPublishEndpoint_P2P::ALWAYS);
    create_endpoint_request
        .set_id("local://grpc-test/responder/play".to_string());
    create_endpoint_request.set_webrtc_pub(endpoint);

    let reply = client
        .create(&create_endpoint_request)
        .expect("create endpoint");
    println!("{:?}", reply);
}

fn delete_room(client: &ControlApiClient) {
    let mut delete_request = IdRequest::new();
    let mut rooms = RepeatedField::new();
    rooms.push("local://pub-sub-video-call".to_string());
    delete_request.set_id(rooms);

    let reply = client.delete(&delete_request).expect("delete room");
    println!("{:?}", reply);
}

fn delete_endpoint(client: &ControlApiClient) {
    let mut delete_endpoint_req = IdRequest::new();
    let mut endpoints = RepeatedField::new();
    endpoints.push("local://video-call-1/caller/publish".to_string());
    delete_endpoint_req.set_id(endpoints);

    let reply = client.delete(&delete_endpoint_req).expect("delete member");
    println!("{:?}", reply);
}

fn delete_member(client: &ControlApiClient) {
    let mut delete_member_req = IdRequest::new();
    let mut members = RepeatedField::new();
    members.push("local://video-call-1/caller".to_string());
    delete_member_req.set_id(members);

    let reply = client.delete(&delete_member_req).expect("delete member");
    println!("{:?}", reply);
}

fn get_room(client: &ControlApiClient) {
    let mut get_room_request = IdRequest::new();
    let mut room = RepeatedField::new();
    room.push("local://grpc-test".to_string());
    room.push("local://video-call-1/responder".to_string());
    room.push("local://grpc-test/publisher/publish".to_string());
    get_room_request.set_id(room);

    let reply = client.get(&get_room_request).expect("get room");
    println!("{:#?}", reply);
}

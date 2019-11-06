use std::time::Duration;

use actix::{Arbiter, Context, System};
use futures::future::Future as _;
use medea_client_api_proto::Event;
use medea_control_api_proto::grpc::{
    api::WebRtcPublishEndpoint_P2P,
    callback::OnLeave_Reason as OnLeaveReasonProto,
};

use crate::{
    callbacks::GetCallbacks,
    gen_insert_str_macro,
    grpc_control_api::{
        ControlClient, MemberBuilder, RoomBuilder, WebRtcPublishEndpointBuilder,
    },
    signalling::{CloseSocket, TestMember},
};

#[test]
fn on_join() {
    gen_insert_str_macro!("member_callback_on_join");
    const CALLBACK_SERVER_PORT: u16 = 9099;

    let sys = System::new(insert_str!("{}"));

    let callback_server = super::run(CALLBACK_SERVER_PORT);
    let control_client = ControlClient::new();
    let member = RoomBuilder::default()
        .id(insert_str!("{}"))
        .add_member(
            MemberBuilder::default()
                .id("publisher")
                .on_join(format!("grpc://127.0.0.1:{}", CALLBACK_SERVER_PORT))
                .build()
                .unwrap(),
        )
        .build()
        .unwrap()
        .build_request(String::new());
    let create_res = control_client.create(&member);

    let on_event =
        move |_: &Event, _: &mut Context<TestMember>, _: Vec<&Event>| {};
    let deadline = Some(Duration::from_secs(5));
    Arbiter::spawn(
        TestMember::connect(
            create_res.get("publisher").unwrap(),
            Box::new(on_event.clone()),
            deadline,
        )
        .and_then(move |_| {
            std::thread::sleep(Duration::from_millis(50));
            callback_server.send(GetCallbacks).map_err(|_| ())
        })
        .map(|callbacks| {
            let callbacks = callbacks.unwrap();
            let on_joins_count = callbacks
                .into_iter()
                .filter(|req| req.has_on_join())
                .count();
            assert_eq!(on_joins_count, 1);
            System::current().stop();
        }),
    );

    sys.run().unwrap()
}

#[test]
fn on_leave() {
    gen_insert_str_macro!("member_callback_on_leave");
    const CALLBACK_SERVER_PORT: u16 = 9098;

    let sys = System::new(insert_str!("{}"));

    let callback_server = super::run(CALLBACK_SERVER_PORT);
    let control_client = ControlClient::new();
    let member = RoomBuilder::default()
        .id(insert_str!("{}"))
        .add_member(
            MemberBuilder::default()
                .id("publisher")
                .on_leave(format!("grpc://127.0.0.1:{}", CALLBACK_SERVER_PORT))
                .build()
                .unwrap(),
        )
        .build()
        .unwrap()
        .build_request(String::new());
    let create_res = control_client.create(&member);

    let on_event =
        move |_: &Event, _: &mut Context<TestMember>, _: Vec<&Event>| {};
    let deadline = Some(Duration::from_secs(5));
    Arbiter::spawn(
        TestMember::connect(
            create_res.get("publisher").unwrap(),
            Box::new(on_event.clone()),
            deadline,
        )
        .and_then(|client| {
            client.send(CloseSocket).map_err(|e| panic!("{:?}", e))
        })
        .and_then(move |_| {
            std::thread::sleep(Duration::from_millis(50));
            callback_server.send(GetCallbacks).map_err(|_| ())
        })
        .map(|callbacks| {
            let callbacks = callbacks.unwrap();
            let on_leaves_count = callbacks
                .into_iter()
                .filter(|req| req.has_on_leave())
                .map(|mut req| req.take_on_leave().reason)
                .filter(|reason| {
                    if let OnLeaveReasonProto::DISCONNECTED = reason {
                        true
                    } else {
                        false
                    }
                })
                .count();
            assert_eq!(on_leaves_count, 1);
            System::current().stop();
        }),
    );

    sys.run().unwrap()
}

#[test]
fn on_leave_on_evicted() {
    gen_insert_str_macro!("member_callback_on_leave_on_evicted");
    const CALLBACK_SERVER_PORT: u16 = 9097;

    let sys = System::new(insert_str!("{}"));

    let callback_server = super::run(CALLBACK_SERVER_PORT);
    let control_client = ControlClient::new();
    let member = RoomBuilder::default()
        .id(insert_str!("{}"))
        .add_member(
            MemberBuilder::default()
                .id("publisher")
                .on_leave(format!("grpc://127.0.0.1:{}", CALLBACK_SERVER_PORT))
                .build()
                .unwrap(),
        )
        .build()
        .unwrap()
        .build_request(String::new());
    let create_res = control_client.create(&member);

    let on_event =
        move |_: &Event, _: &mut Context<TestMember>, _: Vec<&Event>| {};
    let deadline = Some(Duration::from_secs(5));
    Arbiter::spawn(
        TestMember::connect(
            create_res.get("publisher").unwrap(),
            Box::new(on_event.clone()),
            deadline,
        )
        .and_then(move |_| {
            control_client
                .delete(&[&insert_str!("{}/publisher")])
                .unwrap();
            std::thread::sleep(Duration::from_millis(50));
            callback_server.send(GetCallbacks).map_err(|_| ())
        })
        .map(|callbacks| {
            let callbacks = callbacks.unwrap();
            let on_leaves_count = callbacks
                .into_iter()
                .filter(|req| req.has_on_leave())
                .map(|mut req| req.take_on_leave().reason)
                .filter(|reason| {
                    if let OnLeaveReasonProto::EVICTED = reason {
                        true
                    } else {
                        false
                    }
                })
                .count();
            assert_eq!(on_leaves_count, 1);
            System::current().stop();
        }),
    );

    sys.run().unwrap()
}

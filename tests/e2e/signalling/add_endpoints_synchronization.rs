use std::{collections::HashMap, time::Duration};

use futures::{channel::mpsc, StreamExt};
use medea_client_api_proto::{Command, Direction, Event, TrackUpdate};
use medea_control_api_proto::grpc::api::{self as proto};

use crate::{
    grpc_control_api::{
        ControlClient, MemberBuilder, RoomBuilder, WebRtcPlayEndpointBuilder,
        WebRtcPublishEndpointBuilder,
    },
    signalling::{SendCommand, TestMember},
};

pub fn create_room_req(room_id: &str) -> proto::CreateRequest {
    RoomBuilder::default()
        .id(room_id.to_string())
        .add_member(
            MemberBuilder::default()
                .id("first")
                .add_endpoint(
                    WebRtcPublishEndpointBuilder::default()
                        .id("publish")
                        .p2p_mode(proto::web_rtc_publish_endpoint::P2p::Always)
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap(),
        )
        .add_member(
            MemberBuilder::default()
                .id("second")
                .credentials("test")
                .add_endpoint(
                    WebRtcPublishEndpointBuilder::default()
                        .id("publish")
                        .p2p_mode(proto::web_rtc_publish_endpoint::P2p::Always)
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap(),
        )
        .build()
        .unwrap()
        .build_request(String::new())
}

#[actix_rt::test]
async fn add_endpoints_synchronization() {
    const TEST_NAME: &str = "add_endpoints_synchronization";

    let mut client = ControlClient::new().await;
    let credentials = client.create(create_room_req(TEST_NAME)).await;

    let _first = TestMember::connect(
        credentials.get("first").unwrap(),
        None,
        None,
        Some(Duration::from_secs(5)),
        true,
    )
    .await;

    let (second_tx, mut second_rx) = mpsc::unbounded();
    let second = TestMember::connect(
        credentials.get("second").unwrap(),
        Some(Box::new(move |event, _, _| {
            second_tx.unbounded_send(event.clone()).unwrap();
        })),
        None,
        Some(Duration::from_secs(5)),
        false,
    )
    .await;

    let first_play_endpoint = WebRtcPlayEndpointBuilder::default()
        .id("play-second")
        .src(format!("local://{}/second/publish", TEST_NAME))
        .build()
        .unwrap()
        .build_request(format!("{}/first", TEST_NAME));
    client.create(first_play_endpoint).await;

    // Wait for Event::PeerCreated, and create another play endpoint before
    // sending Command::MakeSpdOffer.
    loop {
        if let Event::PeerCreated {
            peer_id,
            negotiation_role: _,
            tracks,
            ..
        } = second_rx.select_next_some().await
        {
            let second_play_endpoint = WebRtcPlayEndpointBuilder::default()
                .id("play-first")
                .src(format!("local://{}/first/publish", TEST_NAME))
                .build()
                .unwrap()
                .build_request(format!("{}/second", TEST_NAME));
            client.create(second_play_endpoint).await;

            let count_send_tracks = tracks
                .iter()
                .filter(|track| match track.direction {
                    Direction::Send { .. } => true,
                    Direction::Recv { .. } => false,
                })
                .count();

            assert_eq!(count_send_tracks, 2);
            assert_eq!(tracks.len(), 2);

            let make_offer = Command::MakeSdpOffer {
                peer_id,
                sdp_offer: "caller_offer".into(),
                mids: tracks
                    .iter()
                    .map(|t| t.id)
                    .enumerate()
                    .map(|(mid, id)| (id, mid.to_string()))
                    .collect(),
                senders_statuses: HashMap::new(),
            };
            second.send(SendCommand(make_offer)).await.unwrap();
            break;
        }
    }

    // Event::SdpAnswerMade must be received before Event::TracksApplied
    loop {
        let event = second_rx.select_next_some().await;
        if let Event::SdpAnswerMade { .. } = event {
            break;
        } else if let Event::TracksApplied { .. } = event {
            panic!("expected Event::SdpAnswerMade");
        }
    }

    // And now we must receive Event::TracksApplied with 2 TrackUpdate::Added
    // with Direction::Recv
    loop {
        let event = second_rx.select_next_some().await;
        if let Event::TracksApplied {
            peer_id: _,
            updates,
            ..
        } = event
        {
            assert_eq!(updates.len(), 2);
            for update in updates {
                match update {
                    TrackUpdate::Added(track) => match track.direction {
                        Direction::Recv { .. } => {}
                        _ => panic!("expected Direction::Recv"),
                    },
                    _ => panic!("expected TrackUpdate::Added"),
                }
            }
            break;
        } else if let Event::TracksApplied { .. } = event {
            panic!("expected Event::SdpAnswerMade");
        }
    }
}
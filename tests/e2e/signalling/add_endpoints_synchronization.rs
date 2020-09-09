use std::time::Duration;

use function_name::named;
use futures::{channel::mpsc, StreamExt};
use medea_client_api_proto::{Direction, Event, PeerUpdate};
use medea_control_api_proto::grpc::api::{self as proto};
use tokio::time::delay_for;

use crate::{
    grpc_control_api::{
        ControlClient, MemberBuilder, RoomBuilder, WebRtcPlayEndpointBuilder,
        WebRtcPublishEndpointBuilder,
    },
    signalling::{handle_peer_created, TestMember},
    test_name,
};

/// Creates Room with two Member's with `WebRtcPublishEndpoint`'s.
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

/// Makes sure that creating endpoints during negotiation works properly.
///
/// 1. Create `Room` with two `Member`'s `WebRtcPublishEndpoint`'s.
/// 2. Connect `Member`'s to `Medea`.
/// 3. Add `WebRtcPlayEndpoint` starting negotiation.
/// 4. Wait for `Event::PeerCreated`.
/// 5. Add second `WebRtcPlayEndpoint`.
/// 6. Answer `Event::PeerCreated` with `Command::MakeSdpOffer`.
/// 7. Make sure that `Event::SdpAnswerMade` is received before
/// `Event::PeerUpdated`, meaning that renegotiation starts only after initial
/// negotiation finishes.
#[actix_rt::test]
#[named]
async fn add_endpoints_synchronization() {
    let mut client = ControlClient::new().await;
    let credentials = client.create(create_room_req(test_name!())).await;

    let _first = TestMember::connect(
        credentials.get("first").unwrap(),
        None,
        None,
        TestMember::DEFAULT_DEADLINE,
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
        TestMember::DEFAULT_DEADLINE,
        false,
    )
    .await;

    let first_play_endpoint = WebRtcPlayEndpointBuilder::default()
        .id("play-second")
        .src(format!("local://{}/second/publish", test_name!()))
        .build()
        .unwrap()
        .build_request(format!("{}/first", test_name!()));
    client.create(first_play_endpoint).await;

    // Wait for Event::PeerCreated, and create another play endpoint before
    // sending Command::MakeSpdOffer.
    loop {
        if let Event::PeerCreated {
            peer_id,
            negotiation_role,
            tracks,
            ..
        } = second_rx.select_next_some().await
        {
            let second_play_endpoint = WebRtcPlayEndpointBuilder::default()
                .id("play-first")
                .src(format!("local://{}/first/publish", test_name!()))
                .build()
                .unwrap()
                .build_request(format!("{}/second", test_name!()));
            client.create(second_play_endpoint).await;

            delay_for(Duration::from_millis(100)).await;

            let count_send_tracks = tracks
                .iter()
                .filter(|track| match track.direction {
                    Direction::Send { .. } => true,
                    Direction::Recv { .. } => false,
                })
                .count();

            assert_eq!(count_send_tracks, 2);
            assert_eq!(tracks.len(), 2);

            second
                .send(handle_peer_created(peer_id, &negotiation_role, &tracks))
                .await
                .unwrap();
            break;
        }
    }

    // Event::SdpAnswerMade must be received before Event::PeerUpdated
    loop {
        let event = second_rx.select_next_some().await;
        if let Event::SdpAnswerMade { .. } = event {
            break;
        } else if let Event::PeerUpdated { .. } = event {
            panic!("expected Event::SdpAnswerMade");
        }
    }

    // And now we must receive Event::PeerUpdated with 2 PeerUpdate::Added
    // with Direction::Recv
    loop {
        let event = second_rx.select_next_some().await;
        if let Event::PeerUpdated {
            peer_id: _,
            updates,
            ..
        } = event
        {
            assert_eq!(updates.len(), 2);
            for update in updates {
                match update {
                    PeerUpdate::Added(track) => match track.direction {
                        Direction::Recv { .. } => {}
                        _ => panic!("expected Direction::Recv"),
                    },
                    _ => panic!("expected PeerUpdate::Added"),
                }
            }
            break;
        } else if let Event::PeerUpdated { .. } = event {
            panic!("expected Event::SdpAnswerMade");
        }
    }
}

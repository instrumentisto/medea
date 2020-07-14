use std::time::Duration;

use futures::{channel::mpsc, StreamExt};
use medea_client_api_proto::{
    Command, Event, NegotiationRole, PeerId, TrackId, TrackPatch, TrackUpdate,
};

use crate::{
    grpc_control_api::{create_room_req, ControlClient},
    signalling::{SendCommand, TestMember},
};

#[actix_rt::test]
async fn track_disable() {
    const TEST_NAME: &str = "track_disable";

    let mut client = ControlClient::new().await;
    let credentials = client.create(create_room_req(TEST_NAME)).await;

    let (publisher_tx, mut publisher_rx) = mpsc::unbounded();
    let publisher = TestMember::connect(
        credentials.get("publisher").unwrap(),
        Some(Box::new(move |event, _, _| {
            publisher_tx.unbounded_send(event.clone()).unwrap();
        })),
        None,
        Some(Duration::from_secs(5)),
        true,
    )
    .await;
    let (subscriber_tx, subscriber_rx) = mpsc::unbounded();
    let subscriber = TestMember::connect(
        credentials.get("responder").unwrap(),
        Some(Box::new(move |event, _, _| {
            subscriber_tx.unbounded_send(event.clone()).unwrap();
        })),
        None,
        Some(Duration::from_secs(5)),
        true,
    )
    .await;

    // wait until initial negotiation finishes
    loop {
        if let Event::SdpAnswerMade { .. } =
            publisher_rx.select_next_some().await
        {
            break;
        };
    }

    // send 2 UpdateTracks with is_muted = true.
    publisher
        .send(SendCommand(Command::UpdateTracks {
            peer_id: PeerId(0),
            tracks_patches: vec![TrackPatch {
                id: TrackId(0),
                is_muted: Some(true),
            }],
        }))
        .await
        .unwrap();
    publisher
        .send(SendCommand(Command::UpdateTracks {
            peer_id: PeerId(0),
            tracks_patches: vec![TrackPatch {
                id: TrackId(1),
                is_muted: Some(true),
            }],
        }))
        .await
        .unwrap();

    // wait for TracksApplied with `is_muted = true` for both tracks
    let mut first_muted = false;
    let mut second_muted = false;
    loop {
        if let Event::TracksApplied {
            peer_id,
            updates,
            negotiation_role,
        } = publisher_rx.select_next_some().await
        {
            assert_eq!(peer_id, PeerId(0));
            assert_eq!(negotiation_role, Some(NegotiationRole::Offerer));
            for update in updates {
                match update {
                    TrackUpdate::Updated(patch) => {
                        if patch.is_muted != Some(true) {
                            unreachable!();
                        }
                        if patch.id == TrackId(0) {
                            first_muted = true;
                        } else if patch.id == TrackId(1) {
                            second_muted = true;
                        } else {
                            unreachable!();
                        }
                    }
                    _ => unreachable!(),
                }
            }
            if first_muted && second_muted {
                break;
            }
        }
    }

    // TODO: assert events received by subscriber
    // TODO: perform renegotiation
}

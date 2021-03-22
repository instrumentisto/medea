use function_name::named;
use futures::{channel::mpsc, StreamExt as _};
use medea_client_api_proto::{
    Command, Event, PeerId, PeerUpdate, TrackId, TrackPatchCommand,
    TrackPatchEvent,
};

use crate::{
    grpc_control_api::{create_room_req, ControlClient},
    if_let_next,
    signalling::{SendCommand, TestMember},
    test_name,
};

/// Checks that if `TrackPatch` only mutes `MediaTrack` then renegotiation
/// wouldn't be started.
#[actix_rt::test]
#[named]
async fn track_mute_doesnt_renegotiates() {
    let mut client = ControlClient::new().await;
    let credentials = client.create(create_room_req(test_name!())).await;

    let (publisher_tx, mut publisher_rx) = mpsc::unbounded();
    let publisher = TestMember::connect(
        credentials.get("publisher").unwrap(),
        Some(Box::new(move |event, _, _| {
            publisher_tx.unbounded_send(event.clone()).unwrap();
        })),
        None,
        TestMember::DEFAULT_DEADLINE,
        true,
        true,
    )
    .await;
    let (subscriber_tx, mut subscriber_rx) = mpsc::unbounded();
    let _subscriber = TestMember::connect(
        credentials.get("responder").unwrap(),
        Some(Box::new(move |event, _, _| {
            subscriber_tx.unbounded_send(event.clone()).unwrap();
        })),
        None,
        TestMember::DEFAULT_DEADLINE,
        true,
        true,
    )
    .await;

    // wait until initial negotiation finishes
    if_let_next! {
        Event::SdpAnswerMade { .. } = publisher_rx {}
    }

    publisher
        .send(SendCommand(Command::UpdateTracks {
            peer_id: PeerId(0),
            tracks_patches: vec![TrackPatchCommand {
                id: TrackId(0),
                enabled: None,
                muted: Some(true),
            }],
        }))
        .await
        .unwrap();

    loop {
        if let Event::PeerUpdated {
            peer_id,
            updates,
            negotiation_role,
        } = publisher_rx.select_next_some().await
        {
            assert_eq!(peer_id, PeerId(0));

            assert!(negotiation_role.is_none());

            assert_eq!(updates.len(), 1);
            assert_eq!(
                updates[0],
                PeerUpdate::Updated(TrackPatchEvent {
                    muted: Some(true),
                    id: TrackId(0),
                    enabled_general: None,
                    enabled_individual: None
                })
            );
            break;
        }
    }

    loop {
        if let Event::PeerUpdated {
            peer_id,
            updates,
            negotiation_role,
        } = subscriber_rx.select_next_some().await
        {
            assert_eq!(peer_id, PeerId(1));

            assert!(negotiation_role.is_none());

            assert_eq!(updates.len(), 1);
            assert_eq!(
                updates[0],
                PeerUpdate::Updated(TrackPatchEvent {
                    muted: Some(true),
                    id: TrackId(0),
                    enabled_general: None,
                    enabled_individual: None
                })
            );
            break;
        }
    }
}

/// Checks that if `TrackPatch` mutes and disables `MediaTrack` then
/// renegotiation will be started despite the `is_muted` field.
#[actix_rt::test]
#[named]
async fn track_mute_with_disable_will_start_renegotiation() {
    let mut client = ControlClient::new().await;
    let credentials = client.create(create_room_req(test_name!())).await;

    let (publisher_tx, mut publisher_rx) = mpsc::unbounded();
    let publisher = TestMember::connect(
        credentials.get("publisher").unwrap(),
        Some(Box::new(move |event, _, _| {
            publisher_tx.unbounded_send(event.clone()).unwrap();
        })),
        None,
        TestMember::DEFAULT_DEADLINE,
        true,
        true,
    )
    .await;
    let (subscriber_tx, mut subscriber_rx) = mpsc::unbounded();
    let _subscriber = TestMember::connect(
        credentials.get("responder").unwrap(),
        Some(Box::new(move |event, _, _| {
            subscriber_tx.unbounded_send(event.clone()).unwrap();
        })),
        None,
        TestMember::DEFAULT_DEADLINE,
        true,
        true,
    )
    .await;

    // wait until initial negotiation finishes
    if_let_next! {
        Event::SdpAnswerMade { .. } = publisher_rx {}
    }

    publisher
        .send(SendCommand(Command::UpdateTracks {
            peer_id: PeerId(0),
            tracks_patches: vec![TrackPatchCommand {
                id: TrackId(0),
                enabled: Some(false),
                muted: Some(true),
            }],
        }))
        .await
        .unwrap();

    loop {
        if let Event::PeerUpdated {
            peer_id,
            updates,
            negotiation_role,
        } = publisher_rx.select_next_some().await
        {
            assert_eq!(peer_id, PeerId(0));

            assert!(negotiation_role.is_some());

            assert_eq!(updates.len(), 1);
            assert_eq!(
                updates[0],
                PeerUpdate::Updated(TrackPatchEvent {
                    muted: Some(true),
                    id: TrackId(0),
                    enabled_general: Some(false),
                    enabled_individual: Some(false),
                })
            );
            break;
        }
    }

    loop {
        if let Event::PeerUpdated {
            peer_id,
            updates,
            negotiation_role,
        } = subscriber_rx.select_next_some().await
        {
            assert_eq!(peer_id, PeerId(1));

            assert!(negotiation_role.is_some());

            assert_eq!(updates.len(), 1);
            assert_eq!(
                updates[0],
                PeerUpdate::Updated(TrackPatchEvent {
                    muted: Some(true),
                    id: TrackId(0),
                    enabled_general: Some(false),
                    enabled_individual: None
                })
            );
            break;
        }
    }
}

use std::time::Duration;

use actix::{Addr, AsyncContext};
use futures::{
    channel::{
        mpsc::{self, UnboundedReceiver},
        oneshot,
    },
    future, StreamExt,
};
use medea_client_api_proto::{
    Command, Event, PeerId, TrackId, TrackPatch, TrackUpdate,
};

use crate::{
    grpc_control_api::{create_room_req, ControlClient},
    signalling::{ConnectionEvent, SendCommand, TestMember},
};
use std::{cell::Cell, rc::Rc};

// Sends 2 UpdateTracks with is_muted = `disabled`.
// Waits for single/multiple TracksApplied with expected track changes on on
// `publisher_rx`.
// Waits for single/multiple TracksApplied with expected track
// changes on on `subscriber_rx`.
async fn helper(
    disabled: bool,
    publisher: &Addr<TestMember>,
    publisher_rx: &mut UnboundedReceiver<Event>,
    subscriber_rx: &mut UnboundedReceiver<Event>,
) {
    // send 2 UpdateTracks with is_muted = true.
    publisher
        .send(SendCommand(Command::UpdateTracks {
            peer_id: PeerId(0),
            tracks_patches: vec![TrackPatch {
                id: TrackId(0),
                is_muted: Some(disabled),
            }],
        }))
        .await
        .unwrap();
    publisher
        .send(SendCommand(Command::UpdateTracks {
            peer_id: PeerId(0),
            tracks_patches: vec![TrackPatch {
                id: TrackId(1),
                is_muted: Some(disabled),
            }],
        }))
        .await
        .unwrap();

    async fn wait_tracks_applied(
        disabled: bool,
        rx: &mut UnboundedReceiver<Event>,
        expected_peer_id: PeerId,
    ) {
        let mut first_muted = false;
        let mut second_muted = false;
        loop {
            if let Event::TracksApplied {
                peer_id, updates, ..
            } = rx.select_next_some().await
            {
                assert_eq!(peer_id, expected_peer_id);
                for update in updates {
                    match update {
                        TrackUpdate::Updated(patch) => {
                            assert_eq!(patch.is_muted, Some(disabled));
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
    };
    wait_tracks_applied(disabled, publisher_rx, PeerId(0)).await;
    wait_tracks_applied(disabled, subscriber_rx, PeerId(1)).await;
}

/// Creates `pub => sub` `Room`, and publisher disables and enables his tracks
/// multiple times.
#[actix_rt::test]
async fn track_disables_and_enables() {
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
        Some(Duration::from_secs(500)),
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
        Some(Duration::from_secs(500)),
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

    helper(true, &publisher, &mut publisher_rx, &mut subscriber_rx).await;
    helper(false, &publisher, &mut publisher_rx, &mut subscriber_rx).await;

    helper(true, &publisher, &mut publisher_rx, &mut subscriber_rx).await;
    helper(false, &publisher, &mut publisher_rx, &mut subscriber_rx).await;
}

/// Tests that track disabled and enables will be performed instantly and will
/// not wait for renegotiation finish.
#[actix_rt::test]
async fn track_disables_and_enables_are_instant() {
    const TEST_NAME: &str = "track_disables_and_enables_are_instant";

    let mut client = ControlClient::new().await;
    let credentials = client.create(create_room_req(TEST_NAME)).await;

    let (publisher_tx, mut publisher_rx) = mpsc::unbounded();
    let _publisher = TestMember::connect(
        credentials.get("publisher").unwrap(),
        Some(Box::new(move |event, _, _| {
            publisher_tx.unbounded_send(event.clone()).unwrap();
        })),
        None,
        Some(Duration::from_secs(500)),
        true,
    )
    .await;
    let (force_update_received_tx, force_update_received_rx) =
        oneshot::channel();
    let mut force_update_received_tx = Some(force_update_received_tx);
    let (all_renegotiations_performed_tx, all_renegotiations_performed_rx) =
        oneshot::channel();
    let mut all_renegotiations_performed_tx =
        Some(all_renegotiations_performed_tx);
    let subscriber = TestMember::connect(
        credentials.get("responder").unwrap(),
        Some(Box::new(move |event, _, _| match event {
            Event::TracksApplied {
                negotiation_role, ..
            } => {
                if negotiation_role.is_none() {
                    if let Some(force_update_received_tx) =
                        force_update_received_tx.take()
                    {
                        let _ = force_update_received_tx.send(());
                    }
                } else {
                    if let Some(all_renegotiations_performed_tx) =
                        all_renegotiations_performed_tx.take()
                    {
                        let _ = all_renegotiations_performed_tx.send(());
                    }
                }
            }
            _ => {}
        })),
        None,
        Some(Duration::from_secs(500)),
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

    async fn send_track_patches(subscriber: &Addr<TestMember>, id: TrackId) {
        let mut futs = Vec::new();
        for i in 0..10 {
            let mut tracks_patches = Vec::new();
            tracks_patches.push(TrackPatch {
                id: id,
                is_muted: Some(i % 2 == 0),
            });
            futs.push(subscriber.send(SendCommand(Command::UpdateTracks {
                peer_id: PeerId(1),
                tracks_patches,
            })));
        }
        future::join_all(futs)
            .await
            .into_iter()
            .for_each(|r| r.unwrap());
    }
    send_track_patches(&subscriber, TrackId(1)).await;
    send_track_patches(&subscriber, TrackId(0)).await;

    let (force_update_received, all_renegotiations_performed) =
        tokio::time::timeout(
            Duration::from_secs(30),
            future::join(
                tokio::time::timeout(
                    Duration::from_secs(10),
                    force_update_received_rx,
                ),
                tokio::time::timeout(
                    Duration::from_secs(10),
                    all_renegotiations_performed_rx,
                ),
            ),
        )
        .await
        .unwrap();
    force_update_received
        .expect("force_update_received")
        .unwrap();
    all_renegotiations_performed
        .expect("all_renegotiations_performed")
        .unwrap();
}

#[actix_rt::test]
async fn force_update_works() {
    const TEST_NAME: &str = "force_update_works";

    let mut client = ControlClient::new().await;
    let credentials = client.create(create_room_req(TEST_NAME)).await;

    let (
        publisher_connection_established_tx,
        mut publisher_connection_established_rx,
    ) = mpsc::unbounded();
    let (force_update_tx, mut force_update_rx) = mpsc::unbounded();
    let (renegotiation_update_tx, mut renegotiation_update_rx) =
        mpsc::unbounded();
    let is_renegotiation_happened = Rc::new(Cell::new(false));
    let _publisher = TestMember::connect(
        credentials.get("publisher").unwrap(),
        Some(Box::new({
            let is_renegotiation_happened =
                Rc::clone(&is_renegotiation_happened);
            let renegotiation_update_tx = renegotiation_update_tx.clone();
            let force_update_tx = force_update_tx.clone();
            move |event, ctx, _| match &event {
                Event::IceCandidateDiscovered { peer_id, .. } => {
                    ctx.notify(SendCommand(Command::UpdateTracks {
                        peer_id: *peer_id,
                        tracks_patches: vec![TrackPatch {
                            is_muted: Some(true),
                            id: TrackId(0),
                        }],
                    }));
                }
                Event::TracksApplied {
                    negotiation_role,
                    peer_id,
                    ..
                } => {
                    if negotiation_role.is_none() {
                        force_update_tx.unbounded_send(()).unwrap();
                    } else {
                        if is_renegotiation_happened.get() {
                            renegotiation_update_tx.unbounded_send(()).unwrap();
                        } else {
                            ctx.notify(SendCommand(Command::UpdateTracks {
                                peer_id: *peer_id,
                                tracks_patches: vec![TrackPatch {
                                    is_muted: Some(true),
                                    id: TrackId(0),
                                }],
                            }));
                            is_renegotiation_happened.set(true);
                        }
                    }
                }
                _ => {}
            }
        })),
        Some(Box::new(move |event| match event {
            ConnectionEvent::Started => publisher_connection_established_tx
                .unbounded_send(())
                .unwrap(),
            _ => (),
        })),
        Some(Duration::from_secs(500)),
        true,
    )
    .await;

    publisher_connection_established_rx.next().await.unwrap();

    let mut pub_peer_id = None;
    let mut track_id = None;
    let _subscriber = TestMember::connect(
        credentials.get("responder").unwrap(),
        Some(Box::new(move |event, ctx, _| match &event {
            Event::PeerCreated {
                peer_id, tracks, ..
            } => {
                track_id = Some(tracks[0].id);
                pub_peer_id = Some(*peer_id);
            }
            Event::IceCandidateDiscovered { .. } => {
                ctx.notify(SendCommand(Command::UpdateTracks {
                    peer_id: pub_peer_id.unwrap(),
                    tracks_patches: vec![TrackPatch {
                        is_muted: Some(true),
                        id: track_id.unwrap(),
                    }],
                }));
            }
            Event::TracksApplied {
                negotiation_role, ..
            } => {
                if negotiation_role.is_none() {
                    force_update_tx.unbounded_send(()).unwrap();
                } else {
                    if is_renegotiation_happened.get() {
                        renegotiation_update_tx.unbounded_send(()).unwrap();
                    } else {
                        ctx.notify(SendCommand(Command::UpdateTracks {
                            peer_id: pub_peer_id.unwrap(),
                            tracks_patches: vec![TrackPatch {
                                is_muted: Some(true),
                                id: track_id.unwrap(),
                            }],
                        }));
                        is_renegotiation_happened.set(true);
                    }
                }
            }
            _ => {}
        })),
        None,
        Some(Duration::from_secs(500)),
        true,
    )
    .await;

    let (force_update, renegotiation_update) = future::join(
        tokio::time::timeout(Duration::from_secs(10), force_update_rx.next()),
        tokio::time::timeout(
            Duration::from_secs(10),
            renegotiation_update_rx.next(),
        ),
    )
    .await;
    force_update.unwrap().unwrap();
    renegotiation_update.unwrap().unwrap();
}

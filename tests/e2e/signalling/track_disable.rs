use std::{cell::Cell, collections::HashMap, rc::Rc, time::Duration};

use actix::{Addr, AsyncContext};
use futures::{
    channel::{
        mpsc::{self, UnboundedReceiver},
        oneshot,
    },
    future, StreamExt,
};
use medea_client_api_proto::{
    Command, Event, NegotiationRole, PeerId, TrackId, TrackPatch, TrackUpdate,
};
use medea_control_api_proto::grpc::api as proto;

use crate::{
    grpc_control_api::{
        create_room_req, ControlClient, WebRtcPlayEndpointBuilder,
        WebRtcPublishEndpointBuilder,
    },
    signalling::{ConnectionEvent, SendCommand, TestMember},
};

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
        Some(Box::new(move |event, ctx, _| {
            if let Event::SdpAnswerMade { peer_id, .. } = event {
                for i in 0..20 {
                    let mut tracks_patches = Vec::new();
                    tracks_patches.push(TrackPatch {
                        id: TrackId(0),
                        is_muted: Some(i % 2 == 0),
                    });
                    ctx.notify(SendCommand(Command::UpdateTracks {
                        peer_id: *peer_id,
                        tracks_patches,
                    }));
                }
            }
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
    let pub_peer_id = Rc::new(Cell::new(None));
    let _subscriber = TestMember::connect(
        credentials.get("responder").unwrap(),
        Some(Box::new({
            let pub_peer_id = Rc::clone(&pub_peer_id);
            move |event, _, _| match event {
                Event::PeerCreated { peer_id, .. } => {
                    pub_peer_id.set(Some(*peer_id));
                }
                Event::TracksApplied {
                    negotiation_role, ..
                } => {
                    if negotiation_role.is_none() {
                        if let Some(force_update_received_tx) =
                            force_update_received_tx.take()
                        {
                            let _ = force_update_received_tx.send(());
                        }
                    } else if let Some(all_renegotiations_performed_tx) =
                        all_renegotiations_performed_tx.take()
                    {
                        all_renegotiations_performed_tx.send(()).unwrap();
                    }
                }
                _ => {}
            }
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
async fn track_disables_and_enables_are_instant2() {
    const TEST_NAME: &str = "track_disables_and_enables_are_instant2";

    let mut client = ControlClient::new().await;
    let credentials = client.create(create_room_req(TEST_NAME)).await;
    client
        .create(
            WebRtcPublishEndpointBuilder::default()
                .id("publish")
                .p2p_mode(proto::web_rtc_publish_endpoint::P2p::Always)
                .build()
                .unwrap()
                .build_request(format!("{}/responder", TEST_NAME)),
        )
        .await;
    client
        .create(
            WebRtcPlayEndpointBuilder::default()
                .id("play")
                .src(format!("local://{}/responder/publish", TEST_NAME))
                .build()
                .unwrap()
                .build_request(format!("{}/publisher", TEST_NAME)),
        )
        .await;

    let (first_tx, mut first_rx) = mpsc::unbounded();
    let first = TestMember::connect(
        credentials.get("publisher").unwrap(),
        Some(Box::new(move |event, _, _| {
            first_tx.unbounded_send(event.clone()).unwrap();
        })),
        None,
        Some(Duration::from_secs(500)),
        false,
    )
    .await;

    let (second_tx, mut second_rx) = mpsc::unbounded();
    let second = TestMember::connect(
        credentials.get("responder").unwrap(),
        Some(Box::new(move |event, _, _| {
            second_tx.unbounded_send(event.clone()).unwrap();
        })),
        None,
        Some(Duration::from_secs(500)),
        false,
    )
    .await;

    loop {
        if let Event::PeerCreated {
            peer_id,
            negotiation_role,
            tracks,
            ..
        } = first_rx.select_next_some().await
        {
            let answer = match negotiation_role {
                NegotiationRole::Offerer => Command::MakeSdpOffer {
                    peer_id,
                    sdp_offer: "offer".into(),
                    mids: tracks
                        .iter()
                        .map(|t| t.id)
                        .enumerate()
                        .map(|(mid, id)| (id, mid.to_string()))
                        .collect(),
                    senders_statuses: HashMap::new(),
                },
                NegotiationRole::Answerer(_) => Command::MakeSdpAnswer {
                    peer_id,
                    sdp_answer: "answer".into(),
                    senders_statuses: HashMap::new(),
                },
            };
            first.send(SendCommand(answer)).await.unwrap();
            break;
        }
    }

    loop {
        if let Event::PeerCreated {
            peer_id,
            negotiation_role,
            tracks,
            ..
        } = second_rx.select_next_some().await
        {
            let answer = match negotiation_role {
                NegotiationRole::Offerer => Command::MakeSdpOffer {
                    peer_id,
                    sdp_offer: "offer".into(),
                    mids: tracks
                        .iter()
                        .map(|t| t.id)
                        .enumerate()
                        .map(|(mid, id)| (id, mid.to_string()))
                        .collect(),
                    senders_statuses: HashMap::new(),
                },
                NegotiationRole::Answerer(_) => Command::MakeSdpAnswer {
                    peer_id,
                    sdp_answer: "answer".into(),
                    senders_statuses: HashMap::new(),
                },
            };
            second.send(SendCommand(answer)).await.unwrap();
            break;
        }
    }
    first
        .send(SendCommand(Command::UpdateTracks {
            peer_id: PeerId(0),
            tracks_patches: vec![TrackPatch {
                id: TrackId(0),
                is_muted: Some(true),
            }],
        }))
        .await
        .unwrap();
    loop {
        if let Event::TracksApplied {
            peer_id,
            updates: _,
            negotiation_role,
        } = first_rx.select_next_some().await
        {
            assert_eq!(peer_id.0, 0);
            assert_eq!(negotiation_role, Some(NegotiationRole::Offerer));
            break;
        }
    }
    second
        .send(SendCommand(Command::UpdateTracks {
            peer_id: PeerId(1),
            tracks_patches: vec![TrackPatch {
                id: TrackId(2),
                is_muted: Some(true),
            }],
        }))
        .await
        .unwrap();
    loop {
        if let Event::TracksApplied {
            peer_id,
            updates: _,
            negotiation_role,
        } = second_rx.select_next_some().await
        {
            assert_eq!(peer_id.0, 1);
            assert_eq!(negotiation_role, None);
            break;
        }
    }
}

/// Checks that force update mechanism works for muting and renegotiation after
/// force update will be performed.
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
                    } else if is_renegotiation_happened.get() {
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
                _ => {}
            }
        })),
        Some(Box::new(move |event| {
            if let ConnectionEvent::Started = event {
                publisher_connection_established_tx
                    .unbounded_send(())
                    .unwrap()
            }
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
                } else if is_renegotiation_happened.get() {
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

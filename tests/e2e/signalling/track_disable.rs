use std::{cell::Cell, rc::Rc, time::Duration};

use actix::{ActorContext, Addr, AsyncContext};
use function_name::named;
use futures::{
    channel::mpsc::{self, UnboundedReceiver},
    future, Stream, StreamExt,
};
use medea_client_api_proto::{
    ClientTrackPatch, Command, Event, NegotiationRole, PeerId, TrackId,
    TrackUpdate,
};
use medea_control_api_proto::grpc::api as proto;
use tokio::time::timeout;

use crate::{
    grpc_control_api::{
        create_room_req, ControlClient, WebRtcPlayEndpointBuilder,
        WebRtcPublishEndpointBuilder,
    },
    signalling::{
        handle_peer_created, ConnectionEvent, SendCommand, TestMember,
    },
    test_name,
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
            tracks_patches: vec![ClientTrackPatch {
                id: TrackId(0),
                is_muted: Some(disabled),
            }],
        }))
        .await
        .unwrap();
    publisher
        .send(SendCommand(Command::UpdateTracks {
            peer_id: PeerId(0),
            tracks_patches: vec![ClientTrackPatch {
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
                            if let Some(is_muted_general) =
                                patch.is_muted_general
                            {
                                assert_eq!(is_muted_general, disabled);
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
    };
    wait_tracks_applied(disabled, publisher_rx, PeerId(0)).await;
    wait_tracks_applied(disabled, subscriber_rx, PeerId(1)).await;
}

/// Creates `pub => sub` `Room`, and publisher disables and enables his tracks
/// multiple times.
#[actix_rt::test]
#[named]
async fn track_disables_and_enables() {
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
#[named]
async fn track_disables_and_enables_are_instant() {
    const EVENTS_COUNT: usize = 100;

    fn filter_events(
        rx: UnboundedReceiver<Event>,
    ) -> impl Stream<Item = (bool, Option<NegotiationRole>)> {
        rx.filter_map(|val| async {
            match val {
                Event::TracksApplied {
                    mut updates,
                    negotiation_role,
                    ..
                } => {
                    match updates.len() {
                        0 => {
                            // 0 updates means that TracksApplied must proc
                            // negotiation
                            negotiation_role.unwrap();
                            None
                        }
                        1 => {
                            if let TrackUpdate::Updated(patch) =
                                updates.pop().unwrap()
                            {
                                Some((
                                    patch.is_muted_general?,
                                    negotiation_role,
                                ))
                            } else {
                                unreachable!();
                            }
                        }
                        _ => unreachable!("patches dedup failed"),
                    }
                }
                _ => None,
            }
        })
    }

    let mut client = ControlClient::new().await;
    let credentials = client.create(create_room_req(test_name!())).await;

    let (publisher_tx, mut publisher_rx) = mpsc::unbounded();
    let publisher = TestMember::connect(
        credentials.get("publisher").unwrap(),
        Some(Box::new(move |event, _, _| {
            let _ = publisher_tx.unbounded_send(event.clone());
        })),
        None,
        TestMember::DEFAULT_DEADLINE,
        true,
    )
    .await;

    let (subscriber_tx, subscriber_rx) = mpsc::unbounded();
    let _subscriber = TestMember::connect(
        credentials.get("responder").unwrap(),
        Some(Box::new(move |event, _, _| {
            let _ = subscriber_tx.unbounded_send(event.clone());
        })),
        None,
        TestMember::DEFAULT_DEADLINE,
        true,
    )
    .await;

    // wait until initial negotiation finishes, and send a bunch of
    // UpdateTracks
    let mut mutes_sent = Vec::with_capacity(EVENTS_COUNT);
    loop {
        if let Event::SdpAnswerMade { .. } =
            publisher_rx.select_next_some().await
        {
            for i in 0..EVENTS_COUNT {
                let is_muted = i % 2 == 1;
                mutes_sent.push(is_muted);
                publisher.do_send(SendCommand(Command::UpdateTracks {
                    peer_id: PeerId(0),
                    tracks_patches: vec![ClientTrackPatch {
                        id: TrackId(0),
                        is_muted: Some(is_muted),
                    }],
                }));
            }
            break;
        };
    }

    // we dont know how many events we will receive, so gather events they
    // stop going
    let mut mutes_received_by_pub: Vec<_> = tokio::stream::StreamExt::timeout(
        filter_events(publisher_rx),
        Duration::from_secs(3),
    )
    .take_while(|val| future::ready(val.is_ok()))
    .map(Result::unwrap)
    .map(|val| val.0)
    .collect()
    .await;

    let mut mutes_received_by_sub: Vec<_> = tokio::stream::StreamExt::timeout(
        filter_events(subscriber_rx),
        Duration::from_secs(3),
    )
    .take_while(|val| future::ready(val.is_ok()))
    .map(Result::unwrap)
    .collect()
    .await;

    let mutes_received_by_pub_len = mutes_received_by_pub.len();
    assert!(mutes_sent.len() >= mutes_received_by_pub_len);

    // make sure that there are no consecutive repeated elements
    mutes_received_by_pub.dedup();
    assert_eq!(mutes_received_by_pub.len(), mutes_received_by_pub_len);

    // make sure that all TracksApplied events received by sub have
    // Some(NegotiationRole), meaning that there no point to force push
    // TracksApplied to other member
    assert!(mutes_received_by_sub.iter().all(|val| val.1.is_some()));

    assert_eq!(
        mutes_sent.pop().unwrap(),
        mutes_received_by_sub.pop().unwrap().0
    );
}

#[actix_rt::test]
#[named]
async fn track_disables_and_enables_are_instant2() {
    let mut client = ControlClient::new().await;
    let credentials = client.create(create_room_req(test_name!())).await;
    client
        .create(
            WebRtcPublishEndpointBuilder::default()
                .id("publish")
                .p2p_mode(proto::web_rtc_publish_endpoint::P2p::Always)
                .build()
                .unwrap()
                .build_request(format!("{}/responder", test_name!())),
        )
        .await;
    client
        .create(
            WebRtcPlayEndpointBuilder::default()
                .id("play")
                .src(format!("local://{}/responder/publish", test_name!()))
                .build()
                .unwrap()
                .build_request(format!("{}/publisher", test_name!())),
        )
        .await;

    let (first_tx, mut first_rx) = mpsc::unbounded();
    let first = TestMember::connect(
        credentials.get("publisher").unwrap(),
        Some(Box::new(move |event, _, _| {
            first_tx.unbounded_send(event.clone()).unwrap();
        })),
        None,
        TestMember::DEFAULT_DEADLINE,
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
        TestMember::DEFAULT_DEADLINE,
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
            first
                .send(handle_peer_created(peer_id, &negotiation_role, &tracks))
                .await
                .unwrap();
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
            second
                .send(handle_peer_created(peer_id, &negotiation_role, &tracks))
                .await
                .unwrap();
            break;
        }
    }
    // wait until initial negotiation finishes
    loop {
        if let Event::SdpAnswerMade { .. } = first_rx.select_next_some().await {
            break;
        };
    }

    first
        .send(SendCommand(Command::UpdateTracks {
            peer_id: PeerId(0),
            tracks_patches: vec![ClientTrackPatch {
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
            tracks_patches: vec![ClientTrackPatch {
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
#[named]
async fn force_update_works() {
    let mut client = ControlClient::new().await;
    let credentials = client.create(create_room_req(test_name!())).await;

    let (pub_con_established_tx, mut pub_con_established_rx) =
        mpsc::unbounded();
    let (force_update_tx, mut force_update_rx) = mpsc::unbounded();
    let (renegotiation_update_tx, mut renegotiation_update_rx) =
        mpsc::unbounded();
    let renegotiation_done = Rc::new(Cell::new(false));
    let _publisher = TestMember::connect(
        credentials.get("publisher").unwrap(),
        Some(Box::new({
            let renegotiation_done = Rc::clone(&renegotiation_done);
            let renegotiation_update_tx = renegotiation_update_tx.clone();
            let force_update_tx = force_update_tx.clone();
            move |event, ctx, _| match &event {
                Event::IceCandidateDiscovered { peer_id, .. } => {
                    ctx.notify(SendCommand(Command::UpdateTracks {
                        peer_id: *peer_id,
                        tracks_patches: vec![ClientTrackPatch {
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
                    } else if renegotiation_done.get() {
                        renegotiation_update_tx.unbounded_send(()).unwrap();
                    } else {
                        ctx.notify(SendCommand(Command::UpdateTracks {
                            peer_id: *peer_id,
                            tracks_patches: vec![ClientTrackPatch {
                                is_muted: Some(true),
                                id: TrackId(0),
                            }],
                        }));
                        renegotiation_done.set(true);
                    }
                }
                _ => {}
            }
        })),
        Some(Box::new(move |event| {
            if let ConnectionEvent::Started = event {
                pub_con_established_tx.unbounded_send(()).unwrap()
            }
        })),
        TestMember::DEFAULT_DEADLINE,
        true,
    )
    .await;

    pub_con_established_rx.next().await.unwrap();

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
                    tracks_patches: vec![ClientTrackPatch {
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
                } else if renegotiation_done.get() {
                    renegotiation_update_tx.unbounded_send(()).unwrap();
                } else {
                    ctx.notify(SendCommand(Command::UpdateTracks {
                        peer_id: pub_peer_id.unwrap(),
                        tracks_patches: vec![ClientTrackPatch {
                            is_muted: Some(true),
                            id: track_id.unwrap(),
                        }],
                    }));
                    renegotiation_done.set(true);
                }
            }
            _ => {}
        })),
        None,
        TestMember::DEFAULT_DEADLINE,
        true,
    )
    .await;

    let (force_update, renegotiation_update) = future::join(
        timeout(Duration::from_secs(10), force_update_rx.next()),
        timeout(Duration::from_secs(10), renegotiation_update_rx.next()),
    )
    .await;
    force_update.unwrap().unwrap();
    renegotiation_update.unwrap().unwrap();
}

use std::sync::atomic::{AtomicU8, Ordering};

/// Checks that server validly switches individual and general mute states based
/// on client's commands.
#[actix_rt::test]
#[named]
async fn individual_and_general_mute_states_works() {
    const STAGE1_PROGRESS: AtomicU8 = AtomicU8::new(0);
    const STAGE2_PROGRESS: AtomicU8 = AtomicU8::new(0);
    const STAGE3_PROGRESS: AtomicU8 = AtomicU8::new(0);

    let mut client = ControlClient::new().await;
    let credentials = client.create(create_room_req(test_name!())).await;

    let (test_finish_tx, test_finish_rx) = mpsc::unbounded();

    let _responder = TestMember::connect(
        credentials.get("responder").unwrap(),
        Some({
            let test_finish_tx = test_finish_tx.clone();
            let mut is_stage1_finished = false;
            let mut is_stage2_finished = false;
            let mut is_stage3_finished = false;

            Box::new(move |event, ctx, _| match event {
                Event::TracksApplied {
                    peer_id, updates, ..
                } => {
                    assert_eq!(peer_id, &PeerId(1));
                    let update = updates.last().unwrap();
                    match update {
                        TrackUpdate::Updated(patch) => {
                            if STAGE1_PROGRESS.load(Ordering::Relaxed) < 2
                                && !is_stage1_finished
                            {
                                assert_eq!(patch.id, TrackId(0));
                                assert_eq!(patch.is_muted_general, Some(true));
                                assert_eq!(patch.is_muted_individual, None);

                                ctx.notify(SendCommand(
                                    Command::UpdateTracks {
                                        peer_id: PeerId(1),
                                        tracks_patches: vec![
                                            ClientTrackPatch {
                                                id: TrackId(0),
                                                is_muted: Some(true),
                                            },
                                        ],
                                    },
                                ));

                                STAGE1_PROGRESS.fetch_add(1, Ordering::Relaxed);
                                is_stage1_finished = true;
                            } else if STAGE2_PROGRESS.load(Ordering::Relaxed)
                                < 2
                                && !is_stage2_finished
                            {
                                assert_eq!(patch.id, TrackId(0));
                                assert_eq!(patch.is_muted_general, Some(true));
                                assert_eq!(
                                    patch.is_muted_individual,
                                    Some(true)
                                );

                                STAGE2_PROGRESS.fetch_add(1, Ordering::Relaxed);
                                is_stage2_finished = true;
                            } else if STAGE3_PROGRESS.load(Ordering::Relaxed)
                                < 2
                                && !is_stage3_finished
                            {
                                assert_eq!(patch.id, TrackId(0));
                                assert_eq!(patch.is_muted_general, None);
                                assert_eq!(patch.is_muted_individual, None);

                                ctx.notify(SendCommand(
                                    Command::UpdateTracks {
                                        peer_id: PeerId(1),
                                        tracks_patches: vec![
                                            ClientTrackPatch {
                                                id: TrackId(0),
                                                is_muted: Some(false),
                                            },
                                        ],
                                    },
                                ));

                                STAGE3_PROGRESS.fetch_add(1, Ordering::Relaxed);
                                is_stage3_finished = true;
                            } else {
                                assert_eq!(patch.id, TrackId(0));
                                assert_eq!(patch.is_muted_general, Some(false));
                                assert_eq!(
                                    patch.is_muted_individual,
                                    Some(false)
                                );

                                test_finish_tx.unbounded_send(()).unwrap();
                                ctx.stop();
                            }
                        }
                        _ => (),
                    }
                }
                _ => (),
            })
        }),
        None,
        TestMember::DEFAULT_DEADLINE,
        true,
    )
    .await;
    let _publisher = TestMember::connect(
        credentials.get("publisher").unwrap(),
        Some(Box::new({
            let mut is_inited = false;
            let mut is_individual_muted = false;
            let mut is_stage1_finished = false;
            let mut is_stage2_finished = false;
            let mut is_stage3_finished = false;

            move |event, ctx, _| match event {
                Event::IceCandidateDiscovered { peer_id, .. } => {
                    assert_eq!(peer_id, &PeerId(0));
                    if !is_inited {
                        ctx.notify(SendCommand(Command::UpdateTracks {
                            peer_id: PeerId(0),
                            tracks_patches: vec![ClientTrackPatch {
                                id: TrackId(0),
                                is_muted: Some(true),
                            }],
                        }));
                        is_inited = true;
                    }
                }
                Event::TracksApplied { updates, .. } => {
                    let update = updates.last().unwrap();
                    match update {
                        TrackUpdate::Updated(patch) => {
                            if STAGE1_PROGRESS.load(Ordering::Relaxed) < 2
                                && !is_stage1_finished
                            {
                                assert_eq!(patch.id, TrackId(0));
                                assert_eq!(
                                    patch.is_muted_individual,
                                    Some(true)
                                );
                                assert_eq!(patch.is_muted_general, Some(true));

                                STAGE1_PROGRESS.fetch_add(1, Ordering::Relaxed);
                                is_stage1_finished = true;
                            } else if STAGE2_PROGRESS.load(Ordering::Relaxed)
                                < 2
                                && !is_stage2_finished
                            {
                                assert_eq!(patch.id, TrackId(0));
                                assert_eq!(patch.is_muted_individual, None);
                                assert_eq!(patch.is_muted_general, Some(true));

                                ctx.notify(SendCommand(
                                    Command::UpdateTracks {
                                        peer_id: PeerId(0),
                                        tracks_patches: vec![
                                            ClientTrackPatch {
                                                id: TrackId(0),
                                                is_muted: Some(false),
                                            },
                                        ],
                                    },
                                ));

                                STAGE2_PROGRESS.fetch_add(1, Ordering::Relaxed);
                                is_stage2_finished = true;
                            } else if STAGE3_PROGRESS.load(Ordering::Relaxed)
                                < 2
                                && is_stage3_finished
                            {
                                assert_eq!(patch.id, TrackId(0));
                                assert_eq!(
                                    patch.is_muted_individual,
                                    Some(false)
                                );
                                assert_eq!(patch.is_muted_general, None);

                                STAGE3_PROGRESS.fetch_add(1, Ordering::Relaxed);
                                is_stage3_finished = true;
                            } else {
                                assert_eq!(patch.id, TrackId(0));
                                if !is_individual_muted {
                                    assert_eq!(
                                        patch.is_muted_individual,
                                        Some(false)
                                    );
                                    assert_eq!(
                                        patch.is_muted_general,
                                        Some(true)
                                    );

                                    is_individual_muted = true;
                                } else {
                                    assert_eq!(
                                        patch.is_muted_general,
                                        Some(false)
                                    );
                                    assert_eq!(patch.is_muted_individual, None);

                                    test_finish_tx.unbounded_send(()).unwrap();

                                    ctx.stop();
                                }
                            }
                        }
                        _ => (),
                    }
                }
                _ => (),
            }
        })),
        None,
        TestMember::DEFAULT_DEADLINE,
        true,
    )
    .await;

    test_finish_rx.skip(1).next().await.unwrap();
}

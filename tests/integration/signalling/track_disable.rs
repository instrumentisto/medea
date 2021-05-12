use std::{
    cell::Cell,
    collections::HashMap,
    rc::Rc,
    sync::atomic::{AtomicU8, Ordering},
    time::Duration,
};

use actix::{ActorContext, Addr, AsyncContext};
use function_name::named;
use futures::{
    channel::mpsc::{self, UnboundedReceiver},
    future, Stream, StreamExt,
};
use medea_client_api_proto::{
    Command, Direction, Event, NegotiationRole, PeerId, PeerUpdate, TrackId,
    TrackPatchCommand,
};
use medea_control_api_proto::grpc::api as proto;
use tokio::time::timeout;

use crate::{
    grpc_control_api::{
        pub_pub_room_req, pub_sub_room_req, ControlClient,
        WebRtcPlayEndpointBuilder, WebRtcPublishEndpointBuilder,
    },
    if_let_next,
    signalling::{
        handle_peer_created, ConnectionEvent, SendCommand, TestMember,
    },
    test_name,
};

// Sends 2 UpdateTracks with provided `enabled`.
// Waits for single/multiple PeerUpdated with expected track changes on on
// `publisher_rx`.
// Waits for single/multiple PeerUpdated with expected track
// changes on on `subscriber_rx`.
async fn helper(
    enabled: bool,
    publisher: &Addr<TestMember>,
    publisher_rx: &mut UnboundedReceiver<Event>,
    subscriber_rx: &mut UnboundedReceiver<Event>,
) {
    // Send 2 UpdateTracks with provided enabled.
    publisher
        .send(SendCommand(Command::UpdateTracks {
            peer_id: PeerId(0),
            tracks_patches: vec![TrackPatchCommand {
                id: TrackId(0),
                enabled: Some(enabled),
                muted: None,
            }],
        }))
        .await
        .unwrap();
    publisher
        .send(SendCommand(Command::UpdateTracks {
            peer_id: PeerId(0),
            tracks_patches: vec![TrackPatchCommand {
                id: TrackId(1),
                enabled: Some(enabled),
                muted: None,
            }],
        }))
        .await
        .unwrap();

    async fn wait_tracks_applied(
        enabled: bool,
        rx: &mut UnboundedReceiver<Event>,
        expected_peer_id: PeerId,
    ) {
        let mut first_disabled = false;
        let mut second_disabled = false;
        loop {
            if let Event::PeerUpdated {
                peer_id, updates, ..
            } = rx.select_next_some().await
            {
                assert_eq!(peer_id, expected_peer_id);
                for update in updates {
                    match update {
                        PeerUpdate::Updated(patch) => {
                            if let Some(enabled_general) = patch.enabled_general
                            {
                                assert_eq!(enabled_general, enabled);
                            } else if let Some(enabled_individual) =
                                patch.enabled_individual
                            {
                                assert_eq!(enabled_individual, enabled);
                            } else {
                                unreachable!()
                            }
                            if patch.id == TrackId(0) {
                                first_disabled = true;
                            } else if patch.id == TrackId(1) {
                                second_disabled = true;
                            } else {
                                unreachable!();
                            }
                        }
                        _ => unreachable!(),
                    }
                }
                if first_disabled && second_disabled {
                    break;
                }
            }
        }
    }
    wait_tracks_applied(enabled, publisher_rx, PeerId(0)).await;
    wait_tracks_applied(enabled, subscriber_rx, PeerId(1)).await;
}

/// Creates `pub => sub` `Room`, and publisher disables and enables his tracks
/// multiple times.
#[actix_rt::test]
#[named]
async fn track_disables_and_enables() {
    let mut client = ControlClient::new().await;
    let credentials = client.create(pub_sub_room_req(test_name!())).await;

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

    helper(false, &publisher, &mut publisher_rx, &mut subscriber_rx).await;
    helper(true, &publisher, &mut publisher_rx, &mut subscriber_rx).await;

    helper(false, &publisher, &mut publisher_rx, &mut subscriber_rx).await;
    helper(true, &publisher, &mut publisher_rx, &mut subscriber_rx).await;
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
                Event::PeerUpdated {
                    mut updates,
                    negotiation_role,
                    ..
                } => {
                    match updates.len() {
                        0 => {
                            // 0 updates means that PeerUpdated must proc
                            // negotiation
                            negotiation_role.unwrap();
                            None
                        }
                        1 => {
                            if let PeerUpdate::Updated(patch) =
                                updates.pop().unwrap()
                            {
                                Some((patch.enabled_general?, negotiation_role))
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
    let credentials = client.create(pub_sub_room_req(test_name!())).await;

    let (publisher_tx, mut publisher_rx) = mpsc::unbounded();
    let publisher = TestMember::connect(
        credentials.get("publisher").unwrap(),
        Some(Box::new(move |event, _, _| {
            let _ = publisher_tx.unbounded_send(event.clone());
        })),
        None,
        TestMember::DEFAULT_DEADLINE,
        true,
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
        true,
    )
    .await;

    // wait until initial negotiation finishes, and send a bunch of
    // UpdateTracks
    let mut mutes_sent = Vec::with_capacity(EVENTS_COUNT);
    if_let_next! {
        Event::SdpAnswerMade { .. } =
            publisher_rx
        {
            for i in 0..EVENTS_COUNT {
                let enabled = i % 2 == 1;
                mutes_sent.push(enabled);
                publisher.do_send(SendCommand(Command::UpdateTracks {
                    peer_id: PeerId(0),
                    tracks_patches: vec![TrackPatchCommand {
                        id: TrackId(0),
                        enabled: Some(enabled),
                        muted: None,
                    }],
                }));
            }
        }
    }

    // we dont know how many events we will receive, so gather events they
    // stop going
    let mut mutes_received_by_pub: Vec<_> = tokio_stream::StreamExt::timeout(
        filter_events(publisher_rx),
        Duration::from_secs(3),
    )
    .take_while(|val| future::ready(val.is_ok()))
    .map(Result::unwrap)
    .map(|val| val.0)
    .collect()
    .await;

    let mut mutes_received_by_sub: Vec<_> = tokio_stream::StreamExt::timeout(
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

    // make sure that all PeerUpdated events received by sub have
    // Some(NegotiationRole), meaning that there no point to force push
    // PeerUpdated to other member
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
    let credentials = client.create(pub_sub_room_req(test_name!())).await;
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
        true,
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
        true,
    )
    .await;

    if_let_next! {
        Event::PeerCreated {
            peer_id,
            negotiation_role,
            tracks,
            ..
        } = first_rx
        {
            first
                .send(handle_peer_created(peer_id, &negotiation_role, &tracks))
                .await
                .unwrap();
        }
    }

    if_let_next! {
        Event::PeerCreated {
            peer_id,
            negotiation_role,
            tracks,
            ..
        } = second_rx
        {
            second
                .send(handle_peer_created(peer_id, &negotiation_role, &tracks))
                .await
                .unwrap();
        }
    }
    // wait until initial negotiation finishes
    if_let_next! {
        Event::SdpAnswerMade { .. } = first_rx {}
    }

    first
        .send(SendCommand(Command::UpdateTracks {
            peer_id: PeerId(0),
            tracks_patches: vec![TrackPatchCommand {
                id: TrackId(0),
                enabled: Some(true),
                muted: None,
            }],
        }))
        .await
        .unwrap();
    if_let_next! {
        Event::PeerUpdated {
            peer_id,
            negotiation_role,
            ..
        } = first_rx
        {
            assert_eq!(peer_id.0, 0);
            assert_eq!(negotiation_role, Some(NegotiationRole::Offerer));
        }
    }
    second
        .send(SendCommand(Command::UpdateTracks {
            peer_id: PeerId(1),
            tracks_patches: vec![TrackPatchCommand {
                id: TrackId(2),
                enabled: Some(true),
                muted: None,
            }],
        }))
        .await
        .unwrap();
    loop {
        if let Event::PeerUpdated {
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
    let credentials = client.create(pub_sub_room_req(test_name!())).await;

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
                        tracks_patches: vec![TrackPatchCommand {
                            enabled: Some(true),
                            id: TrackId(0),
                            muted: None,
                        }],
                    }));
                }
                Event::PeerUpdated {
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
                            tracks_patches: vec![TrackPatchCommand {
                                enabled: Some(true),
                                id: TrackId(0),
                                muted: None,
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
                    tracks_patches: vec![TrackPatchCommand {
                        enabled: Some(true),
                        muted: None,
                        id: track_id.unwrap(),
                    }],
                }));
            }
            Event::PeerUpdated {
                negotiation_role, ..
            } => {
                if negotiation_role.is_none() {
                    force_update_tx.unbounded_send(()).unwrap();
                } else if renegotiation_done.get() {
                    renegotiation_update_tx.unbounded_send(()).unwrap();
                } else {
                    ctx.notify(SendCommand(Command::UpdateTracks {
                        peer_id: pub_peer_id.unwrap(),
                        tracks_patches: vec![TrackPatchCommand {
                            enabled: Some(true),
                            muted: None,
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

/// Checks that bug from [https://github.com/instrumentisto/medea/pull/134]
/// is fixed.
///
/// # Algorithm
///
/// 1. Waits for initial negotiation finish
///
/// 2. Mutes `Alice`'s `Send` `MediaTrack`
///
/// 3. Enables `Alice`'s `Send` `MediaTrack`
///
/// 4. Mutes `Bob`'s `Send` `MediaTrack`
///
/// 5. `Alice` sends SDP offer
///
/// 6. `Bob` should receive [`Event::PeerUpdated`] with empty updates
#[actix_rt::test]
#[named]
async fn ordering_on_force_update_is_correct() {
    let mut client = ControlClient::new().await;
    let credentials = client.create(pub_pub_room_req(test_name!())).await;

    let (alice_events_tx, mut alice_events_rx) = mpsc::unbounded();
    let alice = TestMember::connect(
        credentials.get("alice").unwrap(),
        Some(Box::new(move |event, _, _| {
            alice_events_tx.unbounded_send(event.clone()).unwrap()
        })),
        None,
        None,
        false,
        true,
    )
    .await;
    let (bob_events_tx, mut bob_events_rx) = mpsc::unbounded();
    let bob = TestMember::connect(
        credentials.get("bob").unwrap(),
        Some(Box::new(move |event, _, _| {
            bob_events_tx.unbounded_send(event.clone()).unwrap()
        })),
        None,
        None,
        false,
        true,
    )
    .await;

    let alice_peer_id;
    let alice_sender_id;
    let alice_mids: HashMap<_, _>;
    if_let_next! {
        Event::PeerCreated {
            peer_id,
            negotiation_role,
            tracks,
            ..
        } = alice_events_rx
        {
            alice
                .send(handle_peer_created(peer_id, &negotiation_role, &tracks))
                .await
                .unwrap();
            alice_mids = tracks
                .iter()
                .map(|t| t.id)
                .enumerate()
                .map(|(mid, id)| (id, mid.to_string()))
                .collect();
            alice_sender_id = tracks
                .iter()
                .filter_map(|t| {
                    if let Direction::Send { .. } = t.direction {
                        Some(t.id)
                    } else {
                        None
                    }
                })
                .next()
                .unwrap();
            alice_peer_id = peer_id;
        }
    }

    let bob_peer_id;
    let bob_sender_id;
    if_let_next! {
        Event::PeerCreated {
            peer_id,
            negotiation_role,
            tracks,
            ..
        } = bob_events_rx
        {
            bob.send(handle_peer_created(peer_id, &negotiation_role, &tracks))
                .await
                .unwrap();
            bob_sender_id = tracks.iter()
                .filter_map(|t| {
                    if let Direction::Send { .. } = t.direction {
                        Some(t.id)
                    } else {
                        None
                    }
                })
                .next()
                .unwrap();
            bob_peer_id = peer_id;
        }
    }
    // wait until initial negotiation finishes
    if_let_next! {
        Event::SdpAnswerMade { .. } = alice_events_rx {}
    }

    alice
        .send(SendCommand(Command::UpdateTracks {
            peer_id: alice_peer_id,
            tracks_patches: vec![TrackPatchCommand {
                enabled: Some(true),
                muted: None,
                id: alice_sender_id,
            }],
        }))
        .await
        .unwrap();
    if_let_next! {
        Event::PeerUpdated {
                peer_id,
                negotiation_role,
                mut updates,
        } = alice_events_rx {
            assert_eq!(peer_id, alice_peer_id);
            let update = updates.pop().unwrap();
            if let PeerUpdate::Updated(patch) = update {
                assert_eq!(patch.id, alice_sender_id);
                assert_eq!(patch.enabled_individual, Some(true));
                assert_eq!(patch.enabled_general, Some(true));
            }
            assert_eq!(updates.len(), 0);
            assert_eq!(negotiation_role, Some(NegotiationRole::Offerer));
        }
    }

    alice
        .send(SendCommand(Command::UpdateTracks {
            peer_id: alice_peer_id,
            tracks_patches: vec![TrackPatchCommand {
                enabled: Some(false),
                muted: None,
                id: alice_sender_id,
            }],
        }))
        .await
        .unwrap();
    if_let_next! {
        Event::PeerUpdated {
            peer_id,
            negotiation_role,
            mut updates,
        } = alice_events_rx
        {
            assert_eq!(peer_id, alice_peer_id);
            let update = updates.pop().unwrap();
            if let PeerUpdate::Updated(patch) = update {
                assert_eq!(patch.id, alice_sender_id);
                assert_eq!(patch.enabled_individual, Some(false));
                assert_eq!(patch.enabled_general, Some(false));
            }
            assert_eq!(updates.len(), 0);
            assert_eq!(negotiation_role, None);
        }
    }

    bob.send(SendCommand(Command::UpdateTracks {
        peer_id: bob_peer_id,
        tracks_patches: vec![TrackPatchCommand {
            enabled: Some(true),
            muted: None,
            id: bob_sender_id,
        }],
    }))
    .await
    .unwrap();

    if_let_next! {
        Event::PeerUpdated {
            peer_id,
            negotiation_role,
            updates,
        } = bob_events_rx
        {
            assert_eq!(peer_id, bob_peer_id);
            let mut patches: Vec<_> = updates
                .into_iter()
                .map(|upd| {
                    if let PeerUpdate::Updated(patch) = upd {
                        patch
                    } else {
                        panic!("Expected TrackPatch fount {:?}", upd);
                    }
                })
                .collect();
            patches.sort_by(|a, b| a.id.0.cmp(&b.id.0));

            assert_eq!(patches[1].id, bob_sender_id);
            assert_eq!(patches[1].enabled_individual, Some(true));
            assert_eq!(patches[1].enabled_general, Some(true));

            assert_eq!(patches[0].id, alice_sender_id);
            assert_eq!(patches[0].enabled_individual, None);
            assert_eq!(patches[0].enabled_general, Some(false));

            assert_eq!(patches.len(), 2);
            assert_eq!(negotiation_role, None);
        }
    }

    alice
        .send(SendCommand(Command::MakeSdpOffer {
            peer_id: alice_peer_id,
            sdp_offer: "sdp_offer".to_string(),
            transceivers_statuses: HashMap::new(),
            mids: alice_mids,
        }))
        .await
        .unwrap();

    if_let_next! {
        Event::PeerUpdated {
            peer_id,
            updates,
            negotiation_role,
        } = bob_events_rx
        {
            assert_eq!(peer_id, bob_peer_id);
            assert_eq!(updates.len(), 0);
            assert_eq!(
                negotiation_role,
                Some(NegotiationRole::Answerer("sdp_offer".to_string()))
            );
        }
    }
}

/// Checks that server validly switches individual and general media exchange
/// states based on client's commands.
#[actix_rt::test]
#[named]
async fn individual_and_general_mute_states_works() {
    const STAGE1_PROGRESS: AtomicU8 = AtomicU8::new(0);
    const STAGE2_PROGRESS: AtomicU8 = AtomicU8::new(0);
    const STAGE3_PROGRESS: AtomicU8 = AtomicU8::new(0);

    let mut client = ControlClient::new().await;
    let credentials = client.create(pub_sub_room_req(test_name!())).await;

    let (test_finish_tx, test_finish_rx) = mpsc::unbounded();

    let _responder = TestMember::connect(
        credentials.get("responder").unwrap(),
        Some({
            let test_finish_tx = test_finish_tx.clone();
            let mut stage1_finished = false;
            let mut stage2_finished = false;
            let mut stage3_finished = false;

            Box::new(move |event, ctx, _| match event {
                Event::PeerUpdated {
                    peer_id, updates, ..
                } => {
                    assert_eq!(peer_id, &PeerId(1));
                    let update = updates.last().unwrap();
                    match update {
                        PeerUpdate::Updated(patch) => {
                            if STAGE1_PROGRESS.load(Ordering::Relaxed) < 2
                                && !stage1_finished
                            {
                                assert_eq!(patch.id, TrackId(0));
                                assert_eq!(patch.enabled_general, Some(false));
                                assert_eq!(patch.enabled_individual, None);

                                ctx.notify(SendCommand(
                                    Command::UpdateTracks {
                                        peer_id: PeerId(1),
                                        tracks_patches: vec![
                                            TrackPatchCommand {
                                                id: TrackId(0),
                                                enabled: Some(false),
                                                muted: None,
                                            },
                                        ],
                                    },
                                ));

                                STAGE1_PROGRESS.fetch_add(1, Ordering::Relaxed);
                                stage1_finished = true;
                            } else if STAGE2_PROGRESS.load(Ordering::Relaxed)
                                < 2
                                && !stage2_finished
                            {
                                assert_eq!(patch.id, TrackId(0));
                                assert_eq!(patch.enabled_general, Some(false));
                                assert_eq!(
                                    patch.enabled_individual,
                                    Some(false)
                                );

                                STAGE2_PROGRESS.fetch_add(1, Ordering::Relaxed);
                                stage2_finished = true;
                            } else if STAGE3_PROGRESS.load(Ordering::Relaxed)
                                < 2
                                && !stage3_finished
                            {
                                assert_eq!(patch.id, TrackId(0));
                                assert_eq!(patch.enabled_general, None);
                                assert_eq!(patch.enabled_individual, None);

                                ctx.notify(SendCommand(
                                    Command::UpdateTracks {
                                        peer_id: PeerId(1),
                                        tracks_patches: vec![
                                            TrackPatchCommand {
                                                id: TrackId(0),
                                                enabled: Some(true),
                                                muted: None,
                                            },
                                        ],
                                    },
                                ));

                                STAGE3_PROGRESS.fetch_add(1, Ordering::Relaxed);
                                stage3_finished = true;
                            } else {
                                assert_eq!(patch.id, TrackId(0));
                                assert_eq!(patch.enabled_general, Some(true));
                                assert_eq!(
                                    patch.enabled_individual,
                                    Some(true)
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
        true,
    )
    .await;
    let _publisher = TestMember::connect(
        credentials.get("publisher").unwrap(),
        Some(Box::new({
            let mut is_inited = false;
            let mut enabled_individual = true;
            let mut is_stage1_finished = false;
            let mut is_stage2_finished = false;
            let mut is_stage3_finished = false;

            move |event, ctx, _| match event {
                Event::IceCandidateDiscovered { peer_id, .. } => {
                    assert_eq!(peer_id, &PeerId(0));
                    if !is_inited {
                        ctx.notify(SendCommand(Command::UpdateTracks {
                            peer_id: PeerId(0),
                            tracks_patches: vec![TrackPatchCommand {
                                id: TrackId(0),
                                enabled: Some(false),
                                muted: None,
                            }],
                        }));
                        is_inited = true;
                    }
                }
                Event::PeerUpdated { updates, .. } => {
                    let update = updates.last().unwrap();
                    match update {
                        PeerUpdate::Updated(patch) => {
                            if STAGE1_PROGRESS.load(Ordering::Relaxed) < 2
                                && !is_stage1_finished
                            {
                                assert_eq!(patch.id, TrackId(0));
                                assert_eq!(
                                    patch.enabled_individual,
                                    Some(false)
                                );
                                assert_eq!(patch.enabled_general, Some(false));

                                STAGE1_PROGRESS.fetch_add(1, Ordering::Relaxed);
                                is_stage1_finished = true;
                            } else if STAGE2_PROGRESS.load(Ordering::Relaxed)
                                < 2
                                && !is_stage2_finished
                            {
                                assert_eq!(patch.id, TrackId(0));
                                assert_eq!(patch.enabled_individual, None);
                                assert_eq!(patch.enabled_general, Some(false));

                                ctx.notify(SendCommand(
                                    Command::UpdateTracks {
                                        peer_id: PeerId(0),
                                        tracks_patches: vec![
                                            TrackPatchCommand {
                                                id: TrackId(0),
                                                enabled: Some(true),
                                                muted: None,
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
                                    patch.enabled_individual,
                                    Some(true)
                                );
                                assert_eq!(patch.enabled_general, None);

                                STAGE3_PROGRESS.fetch_add(1, Ordering::Relaxed);
                                is_stage3_finished = true;
                            } else {
                                assert_eq!(patch.id, TrackId(0));
                                if enabled_individual {
                                    assert_eq!(
                                        patch.enabled_individual,
                                        Some(true)
                                    );
                                    assert_eq!(
                                        patch.enabled_general,
                                        Some(false)
                                    );

                                    enabled_individual = false;
                                } else {
                                    assert_eq!(
                                        patch.enabled_general,
                                        Some(true)
                                    );
                                    assert_eq!(patch.enabled_individual, None);

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
        true,
    )
    .await;

    test_finish_rx.skip(1).next().await.unwrap();
}

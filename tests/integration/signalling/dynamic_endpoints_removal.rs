use std::collections::HashMap;

use function_name::named;
use futures::{channel::mpsc, stream::StreamExt as _};
use medea_client_api_proto::{
    Command, Direction, Event, NegotiationRole, PeerUpdate,
};
use medea_control_api_proto::grpc::api as proto;

use crate::{
    grpc_control_api::{
        pub_pub_room_req, pub_sub_room_req, ControlClient,
        WebRtcPlayEndpointBuilder, WebRtcPublishEndpointBuilder,
    },
    if_let_next,
    signalling::{handle_peer_created, SendCommand, TestMember},
    test_name,
};

#[actix_rt::test]
#[named]
async fn delete_and_recreate_play_endpoint() {
    let mut client = ControlClient::new().await;
    let sids = client.create(pub_pub_room_req(test_name!())).await;

    let (alice_events_tx, mut alice_rx) = mpsc::unbounded();
    let alice = TestMember::connect(
        sids.get("alice").unwrap(),
        Some(Box::new(move |event, _, _| {
            alice_events_tx.unbounded_send(event.clone()).unwrap()
        })),
        None,
        None,
        false,
        true,
    )
    .await;
    let (bob_events_tx, mut bob_rx) = mpsc::unbounded();
    let bob = TestMember::connect(
        sids.get("bob").unwrap(),
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
    let mut alice_mids: HashMap<_, _>;
    if_let_next! {
        Event::PeerCreated {
            peer_id,
            negotiation_role,
            tracks,
            ..
        } = alice_rx
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
            alice_peer_id = peer_id;
        }
    }

    let bob_peer_id;
    let mut bob_mids: HashMap<_, _>;
    if_let_next! {
        Event::PeerCreated {
            peer_id,
            negotiation_role,
            tracks,
            ..
        } = bob_rx
        {
            bob.send(handle_peer_created(peer_id, &negotiation_role, &tracks))
                .await
                .unwrap();
            bob_mids = tracks
                .iter()
                .map(|t| t.id)
                .enumerate()
                .map(|(mid, id)| (id, mid.to_string()))
                .collect();
            bob_peer_id = peer_id;
        }
    }
    // wait until initial negotiation finishes
    if_let_next! {
        Event::SdpAnswerMade { .. } = alice_rx {}
    }

    client
        .delete(&[&format!("{}/alice/play", test_name!())])
        .await
        .unwrap();

    let alice_updates;
    if_let_next! {
        Event::PeerUpdated { peer_id, updates, negotiation_role } = alice_rx {
            alice_updates = updates.clone();
            assert_eq!(peer_id, alice_peer_id);
            assert_eq!(3, updates.into_iter().filter(|update| {
                match update {
                    PeerUpdate::Removed(track_id) => {
                        alice_mids.remove(track_id).unwrap();
                        bob_mids.remove(track_id).unwrap();
                        true
                    },
                    _ => false
                }
            }).count());
            assert_eq!(negotiation_role, Some(NegotiationRole::Offerer));

            alice
                .send(SendCommand(Command::MakeSdpOffer {
                    peer_id: alice_peer_id,
                    sdp_offer: String::from("sdp_offer"),
                    transceivers_statuses: HashMap::new(),
                    mids: alice_mids,
                }))
                .await
                .unwrap();
        }
    }
    if_let_next! {
        Event::PeerUpdated { peer_id, updates, negotiation_role } = bob_rx {
            assert_eq!(peer_id, bob_peer_id);
            assert_eq!(updates, alice_updates);
            assert_eq!(
                negotiation_role,
                Some(NegotiationRole::Answerer(String::from("sdp_offer")))
            );

            bob.send(SendCommand(Command::MakeSdpAnswer {
                peer_id: bob_peer_id,
                sdp_answer: String::from("sdp_answer"),
                transceivers_statuses: HashMap::new()
            })).await.unwrap();
        }
    }

    // negotiation after endpoint removal finished
    if_let_next! {
        Event::SdpAnswerMade { .. } = alice_rx {}
    }

    client
        .create(
            WebRtcPlayEndpointBuilder::default()
                .id("play")
                .src(format!("local://{}/bob/publish", test_name!()))
                .build()
                .unwrap()
                .build_request(format!("{}/alice", test_name!())),
        )
        .await;

    if_let_next! {
        Event::PeerUpdated { peer_id, updates, negotiation_role } = bob_rx {
            assert_eq!(peer_id, bob_peer_id);
            assert_eq!(negotiation_role, Some(NegotiationRole::Offerer));
            let updates_count = updates.into_iter().filter(|update| {
                match update {
                    PeerUpdate::Added(track) => match &track.direction {
                        Direction::Send { receivers, ..} => {
                            bob_mids.insert(track.id, track.id.0.to_string());
                            assert_eq!(
                                receivers.get(0).unwrap().0,
                                "alice"
                            );
                            true
                        },
                        _ => false
                    },
                    _ => false
                }
            }).count();
            assert_eq!(updates_count, 3);

            bob
                .send(SendCommand(Command::MakeSdpOffer {
                    peer_id: bob_peer_id,
                    sdp_offer: String::from("sdp_offer"),
                    transceivers_statuses: HashMap::new(),
                    mids: bob_mids,
                }))
                .await
                .unwrap();
        }
    }

    if_let_next! {
        Event::PeerUpdated { peer_id, negotiation_role, .. } = alice_rx {
            assert_eq!(peer_id, alice_peer_id);
            assert_eq!(
                negotiation_role,
                Some(NegotiationRole::Answerer(String::from("sdp_offer")))
            );

            alice.send(SendCommand(Command::MakeSdpAnswer {
                peer_id: alice_peer_id,
                sdp_answer: String::from("sdp_answer"),
                transceivers_statuses: HashMap::new()
            })).await.unwrap();
        }
    }

    if_let_next! {
        Event::SdpAnswerMade { .. } = bob_rx {}
    }
}

#[actix_rt::test]
#[named]
async fn delete_and_recreate_publish_endpoint() {
    let mut client = ControlClient::new().await;
    let sids = client.create(pub_pub_room_req(test_name!())).await;

    let (alice_events_tx, mut alice_rx) = mpsc::unbounded();
    let alice = TestMember::connect(
        sids.get("alice").unwrap(),
        Some(Box::new(move |event, _, _| {
            alice_events_tx.unbounded_send(event.clone()).unwrap()
        })),
        None,
        None,
        false,
        true,
    )
    .await;
    let (bob_events_tx, mut bob_rx) = mpsc::unbounded();
    let bob = TestMember::connect(
        sids.get("bob").unwrap(),
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
    let mut alice_mids: HashMap<_, _>;
    if_let_next! {
        Event::PeerCreated {
            peer_id,
            negotiation_role,
            tracks,
            ..
        } = alice_rx
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
            alice_peer_id = peer_id;
        }
    }

    let bob_peer_id;
    let mut bob_mids: HashMap<_, _>;
    if_let_next! {
        Event::PeerCreated {
            peer_id,
            negotiation_role,
            tracks,
            ..
        } = bob_rx
        {
            bob.send(handle_peer_created(peer_id, &negotiation_role, &tracks))
                .await
                .unwrap();
            bob_mids = tracks
                .iter()
                .map(|t| t.id)
                .enumerate()
                .map(|(mid, id)| (id, mid.to_string()))
                .collect();
            bob_peer_id = peer_id;
        }
    }
    // wait until initial negotiation finishes
    if_let_next! {
        Event::SdpAnswerMade { .. } = alice_rx {}
    }

    client
        .delete(&[&format!("{}/alice/publish", test_name!())])
        .await
        .unwrap();

    let bob_updates;
    if_let_next! {
        Event::PeerUpdated { peer_id, updates, negotiation_role } = bob_rx {
            bob_updates = updates.clone();
            assert_eq!(peer_id, bob_peer_id);
            assert_eq!(3, updates.into_iter().filter(|update| {
                match update {
                    PeerUpdate::Removed(track_id) => {
                        bob_mids.remove(track_id).unwrap();
                        alice_mids.remove(track_id).unwrap();
                        true
                    },
                    _ => false
                }
            }).count());
            assert_eq!(negotiation_role, Some(NegotiationRole::Offerer));

            bob
                .send(SendCommand(Command::MakeSdpOffer {
                    peer_id: bob_peer_id,
                    sdp_offer: String::from("sdp_offer"),
                    transceivers_statuses: HashMap::new(),
                    mids: bob_mids,
                }))
                .await
                .unwrap();
        }
    }
    if_let_next! {
        Event::PeerUpdated { peer_id, updates, negotiation_role } = alice_rx {
            assert_eq!(peer_id, alice_peer_id);
            assert_eq!(updates, bob_updates);
            assert_eq!(
                negotiation_role,
                Some(NegotiationRole::Answerer(String::from("sdp_offer")))
            );

            alice.send(SendCommand(Command::MakeSdpAnswer {
                peer_id: alice_peer_id,
                sdp_answer: String::from("sdp_answer"),
                transceivers_statuses: HashMap::new()
            })).await.unwrap();
        }
    }

    // negotiation after endpoint removal finished
    if_let_next! {
        Event::SdpAnswerMade { .. } = bob_rx {}
    }

    ///////////////////////////////

    client
        .create(
            WebRtcPublishEndpointBuilder::default()
                .id("publish")
                .p2p_mode(proto::web_rtc_publish_endpoint::P2p::Always)
                .build()
                .unwrap()
                .build_request(format!("{}/alice", test_name!())),
        )
        .await;

    client
        .create(
            WebRtcPlayEndpointBuilder::default()
                .id("play")
                .src(format!("local://{}/alice/publish", test_name!()))
                .build()
                .unwrap()
                .build_request(format!("{}/bob", test_name!())),
        )
        .await;

    if_let_next! {
        Event::PeerUpdated { peer_id, updates, negotiation_role } = alice_rx {
            assert_eq!(peer_id, alice_peer_id);
            assert_eq!(negotiation_role, Some(NegotiationRole::Offerer));
            let updates_count = updates.into_iter().filter(|update| {
                match update {
                    PeerUpdate::Added(track) => match &track.direction {
                        Direction::Send { receivers, ..} => {
                            alice_mids.insert(track.id,track.id.0.to_string());
                            assert_eq!(
                                receivers.get(0).unwrap().0,
                                "bob"
                            );
                            true
                        },
                        _ => false
                    },
                    _ => false
                }
            }).count();
            assert_eq!(updates_count, 3);

            alice
                .send(SendCommand(Command::MakeSdpOffer {
                    peer_id: alice_peer_id,
                    sdp_offer: String::from("sdp_offer"),
                    transceivers_statuses: HashMap::new(),
                    mids: alice_mids,
                }))
                .await
                .unwrap();
        }
    }
    if_let_next! {
        Event::PeerUpdated { peer_id, negotiation_role, .. } = bob_rx {
            assert_eq!(peer_id, bob_peer_id);
            assert_eq!(
                negotiation_role,
                Some(NegotiationRole::Answerer(String::from("sdp_offer")))
            );

            bob.send(SendCommand(Command::MakeSdpAnswer {
                peer_id: bob_peer_id,
                sdp_answer: String::from("sdp_answer"),
                transceivers_statuses: HashMap::new()
            })).await.unwrap();
        }
    }

    if_let_next! {
        Event::SdpAnswerMade { .. } = alice_rx {}
    }
}

#[actix_rt::test]
#[named]
async fn delete_and_recreate_single_play_endpoint() {
    let mut client = ControlClient::new().await;
    let sids = client.create(pub_sub_room_req(test_name!())).await;

    let (pub_events_tx, mut pub_events_rx) = mpsc::unbounded();
    let publisher = TestMember::connect(
        sids.get("publisher").unwrap(),
        Some(Box::new(move |event, _, _| {
            pub_events_tx.unbounded_send(event.clone()).unwrap()
        })),
        None,
        None,
        false,
        true,
    )
    .await;
    let (sub_events_tx, mut sub_events_rx) = mpsc::unbounded();
    let subscriber = TestMember::connect(
        sids.get("responder").unwrap(),
        Some(Box::new(move |event, _, _| {
            sub_events_tx.unbounded_send(event.clone()).unwrap()
        })),
        None,
        None,
        false,
        true,
    )
    .await;

    let pub_peer_id;
    if_let_next! {
        Event::PeerCreated {
            peer_id,
            negotiation_role,
            tracks,
            ..
        } = pub_events_rx
        {
            publisher
                .send(handle_peer_created(peer_id, &negotiation_role, &tracks))
                .await
                .unwrap();
            pub_peer_id = peer_id;
        }
    }

    let sub_peer_id;
    if_let_next! {
        Event::PeerCreated {
            peer_id,
            negotiation_role,
            tracks,
            ..
        } = sub_events_rx
        {
            subscriber
                .send(handle_peer_created(peer_id, &negotiation_role, &tracks))
                .await
                .unwrap();
            sub_peer_id = peer_id;
        }
    }
    // wait until initial negotiation finishes
    if_let_next! {
        Event::SdpAnswerMade { .. } = pub_events_rx {}
    }

    client
        .delete(&[&format!("{}/responder/play", test_name!())])
        .await
        .unwrap();

    if_let_next! {
        Event::PeersRemoved { peer_ids } = pub_events_rx {
            assert_eq!(peer_ids, vec![pub_peer_id]);
        }
    }

    if_let_next! {
        Event::PeersRemoved { peer_ids } = sub_events_rx {
            assert_eq!(peer_ids, vec![sub_peer_id]);
        }
    }

    client
        .create(
            WebRtcPlayEndpointBuilder::default()
                .id("play")
                .src(format!("local://{}/publisher/publish", test_name!()))
                .build()
                .unwrap()
                .build_request(format!("{}/responder", test_name!())),
        )
        .await;

    if_let_next! {
        Event::PeerCreated {
            peer_id,
            negotiation_role,
            tracks,
            ..
        } = pub_events_rx
        {
            assert_ne!(peer_id, pub_peer_id);
            publisher
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
        } = sub_events_rx
        {
            assert_ne!(peer_id, sub_peer_id);
            subscriber
                .send(handle_peer_created(peer_id, &negotiation_role, &tracks))
                .await
                .unwrap();
        }
    }

    if_let_next! {
        Event::SdpAnswerMade { .. } = pub_events_rx {}
    }
}

#[actix_rt::test]
#[named]
async fn delete_and_recreate_single_publish_endpoint() {
    let mut client = ControlClient::new().await;
    let sids = client.create(pub_sub_room_req(test_name!())).await;

    let (pub_events_tx, mut pub_events_rx) = mpsc::unbounded();
    let publisher = TestMember::connect(
        sids.get("publisher").unwrap(),
        Some(Box::new(move |event, _, _| {
            pub_events_tx.unbounded_send(event.clone()).unwrap()
        })),
        None,
        None,
        false,
        true,
    )
    .await;
    let (sub_events_tx, mut sub_events_rx) = mpsc::unbounded();
    let subscriber = TestMember::connect(
        sids.get("responder").unwrap(),
        Some(Box::new(move |event, _, _| {
            sub_events_tx.unbounded_send(event.clone()).unwrap()
        })),
        None,
        None,
        false,
        true,
    )
    .await;

    let pub_peer_id;
    if_let_next! {
        Event::PeerCreated {
            peer_id,
            negotiation_role,
            tracks,
            ..
        } = pub_events_rx
        {
            publisher
                .send(handle_peer_created(peer_id, &negotiation_role, &tracks))
                .await
                .unwrap();
            pub_peer_id = peer_id;
        }
    }

    let sub_peer_id;
    if_let_next! {
        Event::PeerCreated {
            peer_id,
            negotiation_role,
            tracks,
            ..
        } = sub_events_rx
        {
            subscriber
                .send(handle_peer_created(peer_id, &negotiation_role, &tracks))
                .await
                .unwrap();
            sub_peer_id = peer_id;
        }
    }
    // wait until initial negotiation finishes
    if_let_next! {
        Event::SdpAnswerMade { .. } = pub_events_rx {}
    }

    client
        .delete(&[&format!("{}/publisher/publish", test_name!())])
        .await
        .unwrap();

    if_let_next! {
        Event::PeersRemoved { peer_ids } = pub_events_rx {
            assert_eq!(peer_ids, vec![pub_peer_id]);
        }
    }

    if_let_next! {
        Event::PeersRemoved { peer_ids } = sub_events_rx {
            assert_eq!(peer_ids, vec![sub_peer_id]);
        }
    }

    client
        .create(
            WebRtcPublishEndpointBuilder::default()
                .id("publish")
                .p2p_mode(proto::web_rtc_publish_endpoint::P2p::Always)
                .build()
                .unwrap()
                .build_request(format!("{}/publisher", test_name!())),
        )
        .await;

    client
        .create(
            WebRtcPlayEndpointBuilder::default()
                .id("play")
                .src(format!("local://{}/publisher/publish", test_name!()))
                .build()
                .unwrap()
                .build_request(format!("{}/responder", test_name!())),
        )
        .await;

    if_let_next! {
        Event::PeerCreated {
            peer_id,
            negotiation_role,
            tracks,
            ..
        } = pub_events_rx
        {
            assert_ne!(peer_id, pub_peer_id);
            publisher
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
        } = sub_events_rx
        {
            assert_ne!(peer_id, sub_peer_id);
            subscriber
                .send(handle_peer_created(peer_id, &negotiation_role, &tracks))
                .await
                .unwrap();
        }
    }

    if_let_next! {
        Event::SdpAnswerMade { .. } = pub_events_rx {}
    }
}

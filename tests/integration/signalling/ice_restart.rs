//! Tests for the ICE restart mechanism.

use std::{cell::RefCell, collections::HashMap, rc::Rc, time::Duration};

use actix::{clock::sleep, Context};
use function_name::named;
use futures::{channel::mpsc::*, StreamExt as _};
use medea::hashmap;
use medea_client_api_proto::{
    Command, Event, NegotiationRole, PeerConnectionState, PeerMetrics,
    PeerUpdate, TrackId,
};

use crate::{
    grpc_control_api::{pub_sub_room_req, ControlClient},
    signalling::{handle_peer_created, SendCommand, TestMember},
    test_name,
};

/// Checks that ICE restarts when `RTCPeerConnection.connectionState` goes to
/// the `Failed` state.
#[actix_rt::test]
#[named]
async fn ice_restart() {
    let control_client = Rc::new(RefCell::new(ControlClient::new().await));
    let credentials = control_client
        .borrow_mut()
        .create(pub_sub_room_req(test_name!()))
        .await;

    let (publisher_tx, mut publisher_rx) = unbounded();
    let publisher = TestMember::connect(
        credentials.get("publisher").unwrap(),
        Some(Box::new(
            move |event: &Event,
                  _: &mut Context<TestMember>,
                  _: Vec<&Event>| {
                publisher_tx.unbounded_send(event.clone()).unwrap();
            },
        )),
        None,
        TestMember::DEFAULT_DEADLINE,
        false,
        true,
    )
    .await;

    let (responder_tx, mut responder_rx) = unbounded();
    let responder = TestMember::connect(
        credentials.get("responder").unwrap(),
        Some(Box::new(
            move |event: &Event,
                  _: &mut Context<TestMember>,
                  _: Vec<&Event>| {
                responder_tx.unbounded_send(event.clone()).unwrap();
            },
        )),
        None,
        TestMember::DEFAULT_DEADLINE,
        false,
        true,
    )
    .await;

    let publisher_peer_id;
    loop {
        if let Event::PeerCreated {
            peer_id,
            negotiation_role,
            tracks,
            ..
        } = publisher_rx.select_next_some().await
        {
            publisher_peer_id = peer_id;
            publisher
                .send(handle_peer_created(peer_id, &negotiation_role, &tracks))
                .await
                .unwrap();
            break;
        }
    }

    let responder_peer_id;
    loop {
        if let Event::PeerCreated {
            peer_id,
            negotiation_role,
            tracks,
            ..
        } = responder_rx.select_next_some().await
        {
            responder_peer_id = peer_id;
            responder
                .send(handle_peer_created(peer_id, &negotiation_role, &tracks))
                .await
                .unwrap();
            break;
        }
    }
    // wait until initial negotiation finishes
    loop {
        if let Event::SdpAnswerMade { .. } =
            publisher_rx.select_next_some().await
        {
            break;
        };
    }

    // first peer connected
    publisher
        .send(SendCommand(Command::AddPeerConnectionMetrics {
            peer_id: publisher_peer_id,
            metrics: PeerMetrics::PeerConnectionState(
                PeerConnectionState::Connected,
            ),
        }))
        .await
        .unwrap();

    // second peer connected
    responder
        .send(SendCommand(Command::AddPeerConnectionMetrics {
            peer_id: responder_peer_id,
            metrics: PeerMetrics::PeerConnectionState(
                PeerConnectionState::Connected,
            ),
        }))
        .await
        .unwrap();

    // first peer failed
    publisher
        .send(SendCommand(Command::AddPeerConnectionMetrics {
            peer_id: publisher_peer_id,
            metrics: PeerMetrics::PeerConnectionState(
                PeerConnectionState::Failed,
            ),
        }))
        .await
        .unwrap();

    // make sure that there are not contention between Failed messages
    sleep(Duration::from_millis(500)).await;

    // second peer failed
    responder
        .send(SendCommand(Command::AddPeerConnectionMetrics {
            peer_id: responder_peer_id,
            metrics: PeerMetrics::PeerConnectionState(
                PeerConnectionState::Failed,
            ),
        }))
        .await
        .unwrap();

    {
        let event = responder_rx.next().await.unwrap();
        if let Event::LocalDescriptionApplied { peer_id, .. } = event {
            assert_eq!(peer_id, responder_peer_id);
        } else {
            unreachable!(
                "Received {:?} instead of Event::SdpOfferApplied",
                event
            )
        }
    }

    {
        let event = responder_rx.next().await.unwrap();
        match event {
            Event::PeerUpdated {
                peer_id,
                updates,
                negotiation_role,
            } => {
                assert_eq!(peer_id, responder_peer_id);
                assert_eq!(negotiation_role, Some(NegotiationRole::Offerer));
                let is_ice_restart = updates
                    .iter()
                    .find(|upd| matches!(upd, PeerUpdate::IceRestart))
                    .is_some();
                assert!(is_ice_restart);
            }
            _ => unreachable!(
                "Received {:?} instead of Event::PeerCreated",
                event
            ),
        }
    }

    responder
        .send(SendCommand(Command::MakeSdpOffer {
            peer_id: responder_peer_id,
            transceivers_statuses: HashMap::new(),
            sdp_offer: String::from("offer"),
            mids: hashmap! {
                TrackId(0) => String::from("0"),
                TrackId(1) => String::from("1"),
                TrackId(2) => String::from("2"),
            },
        }))
        .await
        .unwrap();

    {
        let event = publisher_rx.next().await.unwrap();
        match event {
            Event::PeerUpdated {
                peer_id,
                updates: _,
                negotiation_role,
            } => {
                assert_eq!(peer_id, publisher_peer_id);
                if let Some(NegotiationRole::Answerer(sdp_offer)) =
                    negotiation_role
                {
                    assert_eq!(sdp_offer, String::from("offer"));
                } else {
                    panic!(
                        "Negotiation role is not Asnwerer: {:?}",
                        negotiation_role
                    );
                }
            }
            _ => unreachable!(
                "Received {:?} instead of Event::PeerCreated",
                event
            ),
        }
    }

    // first peer answers with SDP answer
    publisher
        .send(SendCommand(Command::MakeSdpAnswer {
            peer_id: publisher_peer_id,
            sdp_answer: String::from("answer"),
            transceivers_statuses: HashMap::new(),
        }))
        .await
        .unwrap();

    {
        let event = responder_rx.next().await.unwrap();
        if let Event::LocalDescriptionApplied { peer_id, .. } = event {
            assert_eq!(peer_id, responder_peer_id);
        } else {
            unreachable!(
                "Received {:?} instead of Event::SdpOfferApplied",
                event
            )
        }
    }

    // second peer receives answer
    {
        let event = responder_rx.next().await.unwrap();
        match event {
            Event::SdpAnswerMade {
                peer_id,
                sdp_answer,
            } => {
                assert_eq!(peer_id, responder_peer_id);
                assert_eq!(sdp_answer, String::from("answer"));
            }
            _ => unreachable!(
                "Received {:?} instead of Event::PeerCreated",
                event
            ),
        }
    }
}

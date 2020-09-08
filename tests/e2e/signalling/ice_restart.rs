use std::{cell::RefCell, collections::HashMap, rc::Rc};

use actix::Context;
use function_name::named;
use futures::{channel::mpsc::*, StreamExt as _};
use medea::hashmap;
use medea_client_api_proto::{
    Command, Event, NegotiationRole, PeerConnectionState, PeerMetrics,
    PeerUpdate, TrackId,
};

use crate::{
    grpc_control_api::{create_room_req, ControlClient},
    signalling::{handle_peer_created, SendCommand, TestMember},
    test_name,
};

#[actix_rt::test]
#[named]
async fn ice_restart() {
    let control_client = Rc::new(RefCell::new(ControlClient::new().await));
    let credentials = control_client
        .borrow_mut()
        .create(create_room_req(test_name!()))
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

    match responder_rx.next().await.unwrap() {
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
        _ => unreachable!(),
    }

    responder
        .send(SendCommand(Command::MakeSdpOffer {
            peer_id: responder_peer_id,
            transceivers_statuses: HashMap::new(),
            sdp_offer: String::from("offer"),
            mids: hashmap! {
                TrackId(0) => String::from("0"),
                TrackId(1) => String::from("1"),
            },
        }))
        .await
        .unwrap();

    match publisher_rx.next().await.unwrap() {
        Event::PeerUpdated {
            peer_id,
            updates: _,
            negotiation_role,
        } => {
            assert_eq!(peer_id, publisher_peer_id);
            if let Some(NegotiationRole::Answerer(sdp_offer)) = negotiation_role
            {
                assert_eq!(sdp_offer, String::from("offer"));
            } else {
                panic!(
                    "Negotiation role is not Asnwerer: {:?}",
                    negotiation_role
                );
            }
        }
        _ => unreachable!(),
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

    // second peer receives answer
    match responder_rx.next().await.unwrap() {
        Event::SdpAnswerMade {
            peer_id,
            sdp_answer,
        } => {
            assert_eq!(peer_id, responder_peer_id);
            assert_eq!(sdp_answer, String::from("answer"));
        }
        _ => unreachable!(),
    }
}

use actix::{Context, System};
use medea_client_api_proto::{Direction, Event};

use crate::signalling::TestMember;

#[test]
fn pub_sub_video_call() {
    System::run(|| {
        let base_url = "ws://127.0.0.1:8080/ws/pub-sub-video-call";

        // Note that events is separated by members.
        // Every member will have different instance of this.
        let test_fn = move |event: &Event,
                            _: &mut Context<TestMember>,
                            events: Vec<&Event>| {
            // Start checking result of test.
            if let Event::IceCandidateDiscovered { .. } = event {
                let peers_count = events
                    .iter()
                    .filter(|e| match e {
                        Event::PeerCreated { .. } => true,
                        _ => false,
                    })
                    .count();
                assert_eq!(peers_count, 1);

                let is_caller;
                if let Event::PeerCreated {
                    peer_id,
                    sdp_offer,
                    tracks,
                    ice_servers,
                    is_relay,
                } = &events[0]
                {
                    assert_eq!(ice_servers.len(), 2);
                    assert_eq!(
                        ice_servers[0].urls[0],
                        "stun:localhost:3478".to_string()
                    );
                    assert_eq!(
                        ice_servers[1].urls[0],
                        "turn:localhost:3478".to_string()
                    );
                    assert_eq!(
                        ice_servers[1].urls[1],
                        "turn:localhost:3478?transport=tcp".to_string()
                    );
                    assert_eq!(is_relay, &true);

                    if sdp_offer.is_some() {
                        is_caller = false;
                    } else {
                        is_caller = true;
                    }
                    assert_eq!(tracks.len(), 2);
                    for track in tracks {
                        match &track.direction {
                            Direction::Send { receivers, .. } => {
                                assert!(is_caller);
                                assert!(!receivers.contains(&peer_id));
                            }
                            Direction::Recv { sender, .. } => {
                                assert!(!is_caller);
                                assert_ne!(sender, peer_id);
                            }
                        }
                    }
                } else {
                    unreachable!()
                }

                if is_caller {
                    if let Event::SdpAnswerMade { .. } = &events[1] {
                    } else {
                        unreachable!();
                    }

                    if let Event::IceCandidateDiscovered { .. } = &events[2] {
                    } else {
                        unreachable!();
                    }
                } else if let Event::IceCandidateDiscovered { .. } = &events[1]
                {
                } else {
                    unreachable!();
                }

                if is_caller {
                    System::current().stop();
                }
            }
        };

        let deadline = Some(std::time::Duration::from_secs(5));
        TestMember::start(
            &format!("{}/caller/test", base_url),
            Box::new(test_fn),
            deadline,
        );
        TestMember::start(
            &format!("{}/responder/test", base_url),
            Box::new(test_fn),
            deadline,
        );
    })
    .unwrap();
}

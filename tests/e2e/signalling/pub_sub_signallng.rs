use std::collections::HashSet;

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
            let peers_count = events
                .iter()
                .filter(|e| match e {
                    Event::PeerCreated { .. } => true,
                    _ => false,
                })
                .count();
            if peers_count < 1 {
                return;
            }
            assert_eq!(peers_count, 1);

            // Start checking result of test.
            if let Event::IceCandidateDiscovered { .. } = event {
                let is_caller;
                if let Event::PeerCreated {
                    peer_id,
                    sdp_offer,
                    tracks,
                    ice_servers,
                    force_relay,
                } = &events[0]
                {
                    assert_eq!(ice_servers.len(), 2);
                    let urls: HashSet<_> = ice_servers
                        .iter()
                        .flat_map(|i| i.urls.iter().cloned())
                        .collect();
                    assert!(urls.contains("stun:localhost:3478"));
                    assert!(urls.contains("turn:localhost:3478"));
                    assert!(urls.contains("turn:localhost:3478?transport=tcp"));
                    assert_eq!(force_relay, &true);

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
            format!("{}/caller/test", base_url),
            Some(Box::new(test_fn)),
            None,
            deadline,
        );
        TestMember::start(
            format!("{}/responder/test", base_url),
            Some(Box::new(test_fn)),
            None,
            deadline,
        );
    })
    .unwrap();
}

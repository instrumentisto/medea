use std::{cell::Cell, rc::Rc};

use actix::{AsyncContext as _, Context, System};
use actix_http::ws::CloseCode;
use medea_client_api_proto::{Direction, Event};

use crate::signalling::{CloseSocket, TestMember};

#[test]
#[allow(clippy::too_many_lines)]
fn three_members_p2p_video_call() {
    System::run(|| {
        let base_url = "ws://127.0.0.1:8080/ws/three-members-conference";

        // Note that events, peer_created_count, ice_candidates
        // is separated by members.
        // Every member will have different instance of this.
        let mut peer_created_count = 0;
        let mut ice_candidates = 0;

        // This is shared state of members.
        let members_tested = Rc::new(Cell::new(0));
        let members_peers_removed = Rc::new(Cell::new(0));

        let test_fn = move |event: &Event,
                            ctx: &mut Context<TestMember>,
                            events: Vec<&Event>| {
            match event {
                Event::PeerCreated { ice_servers, .. } => {
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

                    peer_created_count += 1;
                }
                Event::IceCandidateDiscovered { .. } => {
                    ice_candidates += 1;
                }
                Event::PeersRemoved { .. } => {
                    // This event should get two remaining members after closing
                    // last tested member.
                    let peers_removed: Vec<_> = events
                        .iter()
                        .filter_map(|e| match e {
                            Event::PeersRemoved { peer_ids } => Some(peer_ids),
                            _ => None,
                        })
                        .collect();
                    assert_eq!(peers_removed.len(), 1);
                    assert_eq!(peers_removed[0].len(), 1);

                    members_peers_removed.set(members_peers_removed.get() + 1);
                    // Stop when all members receive Event::PeerRemoved
                    if members_peers_removed.get() == 2 {
                        System::current().stop();
                    }
                }
                _ => (),
            }

            if ice_candidates == 2 && peer_created_count >= 2 {
                // Start checking result of test.

                assert_eq!(peer_created_count, 2);

                events.iter().for_each(|e| {
                    if let Event::PeerCreated { tracks, .. } = e {
                        assert_eq!(tracks.len(), 6);
                        let recv_count = tracks
                            .iter()
                            .filter_map(|t| match &t.direction {
                                Direction::Recv { sender, .. } => Some(sender),
                                _ => None,
                            })
                            .count();
                        assert_eq!(recv_count, 3);

                        let send_count = tracks
                            .iter()
                            .filter_map(|t| match &t.direction {
                                Direction::Send { receivers, .. } => {
                                    assert_eq!(receivers.len(), 1);
                                    Some(receivers)
                                }
                                _ => None,
                            })
                            .count();
                        assert_eq!(send_count, 3);
                    }
                });

                // Check peers removing.
                // After closing socket, server should send
                // Event::PeersRemoved to all remaining
                // members.
                // Close should happen when last TestMember pass
                // tests.
                if members_tested.get() == 2 {
                    ctx.notify(CloseSocket(CloseCode::Normal));
                }
                members_tested.set(members_tested.get() + 1);
            }
        };

        let deadline = Some(std::time::Duration::from_secs(5));
        TestMember::start(
            format!("{}/member-1?token=test", base_url),
            Some(Box::new(test_fn.clone())),
            None,
            deadline,
        );
        TestMember::start(
            format!("{}/member-2?token=test", base_url),
            Some(Box::new(test_fn.clone())),
            None,
            deadline,
        );
        TestMember::start(
            format!("{}/member-3?token=test", base_url),
            Some(Box::new(test_fn)),
            None,
            deadline,
        );
    })
    .unwrap();
}

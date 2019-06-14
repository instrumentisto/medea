//! Signalling API e2e tests.

// TODO: dockerize app, run tests on single instance, remove e2e_tests feature, run tests with redis up, extend tests with ice servers check

use std::{cell::Cell, rc::Rc, time::Duration};

use actix::{
    Actor, Arbiter, AsyncContext, Context, Handler, Message, StreamHandler,
    System,
};
use actix_web::ws::{
    Client, ClientWriter, CloseCode, CloseReason, Message as WebMessage,
    ProtocolError,
};
use futures::future::Future;
use medea::media::PeerId;
use medea_client_api_proto::{Command, Direction, Event, IceCandidate};
use serde_json::error::Error as SerdeError;

/// Medea client for testing purposes.
struct TestMember {
    /// Writer to WebSocket.
    writer: ClientWriter,

    /// All [`Event`]s which this [`TestMember`] received.
    /// This field used for give some debug info when test just stuck forever
    /// (most often, such a test will end on a timer of five seconds
    /// and display all events of this [`TestMember`]).
    events: Vec<Event>,

    /// Function which will be called at every received by this [`TestMember`]
    /// [`Event`].
    test_fn: Box<FnMut(&Event, &mut Context<TestMember>)>,
}

impl TestMember {
    /// Signaling heartbeat for server.
    /// Most likely, this ping will never be sent,
    /// because it has been established that it is sent once per 3 seconds,
    /// and there are simply no tests that last so much.
    fn heartbeat(&self, ctx: &mut Context<Self>) {
        ctx.run_later(Duration::from_secs(3), |act, ctx| {
            act.writer.text(r#"{"ping": 1}"#);
            act.heartbeat(ctx);
        });
    }

    /// Send command to the server.
    fn send_command(&mut self, msg: Command) {
        self.writer.text(&serde_json::to_string(&msg).unwrap());
    }

    /// Start test member in new [`Arbiter`] by given URI.
    /// `test_fn` - is function which will be called at every [`Event`]
    /// received from server.
    pub fn start(
        uri: &str,
        test_fn: Box<FnMut(&Event, &mut Context<TestMember>)>,
    ) {
        Arbiter::spawn(
            Client::new(uri)
                .connect()
                .map_err(|e| {
                    panic!("Error: {}", e);
                })
                .map(|(reader, writer)| {
                    TestMember::create(|ctx| {
                        TestMember::add_stream(reader, ctx);
                        TestMember {
                            writer,
                            events: Vec::new(),
                            test_fn,
                        }
                    });
                }),
        )
    }
}

impl Actor for TestMember {
    type Context = Context<Self>;

    /// Start heartbeat and set a timer that will panic when 5 seconds expire.
    /// The timer is needed because some tests may just stuck
    /// and listen socket forever.
    fn started(&mut self, ctx: &mut Self::Context) {
        self.heartbeat(ctx);
        ctx.run_later(Duration::from_secs(5), |act, _ctx| {
            panic!(
                "This test lasts more than 5 seconds. Most likely, this is \
                 not normal. Here is all events of member: {:?}",
                act.events
            );
        });
    }
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
struct CloseSocket;

impl Handler<CloseSocket> for TestMember {
    type Result = ();

    fn handle(&mut self, _: CloseSocket, _: &mut Self::Context) {
        self.writer.close(Some(CloseReason {
            code: CloseCode::Normal,
            description: None,
        }));
    }
}

impl StreamHandler<WebMessage, ProtocolError> for TestMember {
    /// Basic signalling implementation.
    /// A `TestMember::test_fn` [`FnMut`] function will be called for each
    /// [`Event`] received from test server.
    fn handle(&mut self, msg: WebMessage, ctx: &mut Context<Self>) {
        match msg {
            WebMessage::Text(txt) => {
                let event: Result<Event, SerdeError> =
                    serde_json::from_str(&txt);
                if let Ok(event) = event {
                    self.events.push(event.clone());
                    // Test function call
                    (self.test_fn)(&event, ctx);
                    match event {
                        Event::PeerCreated {
                            peer_id, sdp_offer, ..
                        } => {
                            match sdp_offer {
                                Some(_) => {
                                    self.send_command(Command::MakeSdpAnswer {
                                        peer_id,
                                        sdp_answer: "responder_answer"
                                            .to_string(),
                                    });
                                }
                                None => {
                                    self.send_command(Command::MakeSdpOffer {
                                        peer_id,
                                        sdp_offer: "caller_offer".to_string(),
                                    });
                                }
                            }
                            self.send_command(Command::SetIceCandidate {
                                peer_id,
                                candidate: IceCandidate {
                                    candidate: "ice_candidate".to_string(),
                                    sdp_m_line_index: None,
                                    sdp_mid: None,
                                },
                            });
                        }
                        _ => (),
                    }
                }
            }
            _ => (),
        }
    }
}

#[test]
fn pub_sub_video_call() {
    let test_name = "pub_sub_video_call";
    let base_url = "ws://localhost:8081/ws/pub-sub-video-call";

    let sys = System::new(test_name);

    // Note that events is separated by members.
    // Every member will have different instance of this.
    let mut events = Vec::new();
    let test_fn = move |event: &Event, _: &mut Context<TestMember>| {
        events.push(event.clone());

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

            let mut is_caller = false;
            if let Event::PeerCreated {
                peer_id,
                sdp_offer,
                tracks,
                ice_servers,
            } = &events[0]
            {
                assert_eq!(ice_servers.len(), 2);

                if let Some(_) = sdp_offer {
                    is_caller = false;
                } else {
                    is_caller = true;
                }
                assert_eq!(tracks.len(), 2);
                for track in tracks {
                    match &track.direction {
                        Direction::Send { receivers } => {
                            assert!(is_caller);
                            assert!(!receivers.contains(&peer_id));
                        }
                        Direction::Recv { sender } => {
                            assert!(!is_caller);
                            assert_ne!(sender, peer_id);
                        }
                    }
                }
            } else {
                assert!(false)
            }

            if is_caller {
                if let Event::SdpAnswerMade { .. } = &events[1] {
                } else {
                    assert!(false);
                }

                if let Event::IceCandidateDiscovered { .. } = &events[2] {
                } else {
                    assert!(false);
                }
            } else {
                if let Event::IceCandidateDiscovered { .. } = &events[1] {
                } else {
                    assert!(false);
                }
            }

            if is_caller {
                System::current().stop();
            }
        }
    };

    TestMember::start(
        &format!("{}/caller/test", base_url),
        Box::new(test_fn.clone()),
    );
    TestMember::start(
        &format!("{}/responder/test", base_url),
        Box::new(test_fn),
    );

    let _ = sys.run();
}

#[test]
fn three_members_p2p_video_call() {
    let test_name = "three_members_p2p_video_call";

    let base_url = "ws://localhost:8081/ws/three-members-conference";

    let sys = System::new(test_name);

    // Note that events, peer_created_count, ice_candidates
    // is separated by members.
    // Every member will have different instance of this.
    let mut events = Vec::new();
    let mut peer_created_count = 0;
    let mut ice_candidates = 0;

    // This is shared state of members.
    let members_tested = Rc::new(Cell::new(0));
    let members_peers_removed = Rc::new(Cell::new(0));

    let test_fn = move |event: &Event, ctx: &mut Context<TestMember>| {
        events.push(event.clone());
        match event {
            Event::PeerCreated { ice_servers, .. } => {
                assert_eq!(ice_servers.len(), 2);
                peer_created_count += 1;
            }
            Event::IceCandidateDiscovered { .. } => {
                ice_candidates += 1;
                if ice_candidates == 2 {
                    // Start checking result of test.

                    assert_eq!(peer_created_count, 2);

                    events.iter().for_each(|e| match e {
                        Event::PeerCreated {
                            peer_id, tracks, ..
                        } => {
                            assert_eq!(tracks.len(), 4);
                            let recv_count = tracks
                                .iter()
                                .filter_map(|t| match &t.direction {
                                    Direction::Recv { sender } => Some(sender),
                                    _ => None,
                                })
                                .map(|sender| {
                                    assert_ne!(sender, peer_id);
                                })
                                .count();
                            assert_eq!(recv_count, 2);

                            let send_count = tracks
                                .iter()
                                .filter_map(|t| match &t.direction {
                                    Direction::Send { receivers } => {
                                        Some(receivers)
                                    }
                                    _ => None,
                                })
                                .map(|receivers| {
                                    assert!(!receivers.contains(peer_id));
                                    assert_eq!(receivers.len(), 1);
                                })
                                .count();
                            assert_eq!(send_count, 2);
                        }
                        _ => (),
                    });

                    // Check peers removing.
                    // After closing socket, server should send
                    // Event::PeersRemoved to all remaining
                    // members.
                    // Close should happen when last TestMember pass
                    // tests.
                    if members_tested.get() == 2 {
                        ctx.notify(CloseSocket);
                    }
                    members_tested.set(members_tested.get() + 1);
                }
            }
            Event::PeersRemoved { .. } => {
                // This event should get two remaining members after closing
                // last tested member.
                let peers_removed: Vec<&Vec<PeerId>> = events
                    .iter()
                    .filter_map(|e| match e {
                        Event::PeersRemoved { peer_ids } => Some(peer_ids),
                        _ => None,
                    })
                    .collect();
                assert_eq!(peers_removed.len(), 1);
                assert_eq!(peers_removed[0].len(), 2);
                assert_ne!(peers_removed[0][0], peers_removed[0][1]);

                members_peers_removed.set(members_peers_removed.get() + 1);
                // Stop when all members receive Event::PeerRemoved
                if members_peers_removed.get() == 2 {
                    System::current().stop();
                }
            }
            _ => (),
        }
    };

    TestMember::start(
        &format!("{}/member-1/test", base_url),
        Box::new(test_fn.clone()),
    );
    TestMember::start(
        &format!("{}/member-2/test", base_url),
        Box::new(test_fn.clone()),
    );
    TestMember::start(
        &format!("{}/member-3/test", base_url),
        Box::new(test_fn),
    );

    let _ = sys.run();
}

mod test_ports;

use actix::{Actor, Arbiter, AsyncContext, Context, StreamHandler, System};
use actix_web::ws::{
    Client, ClientWriter, Message as WebMessage, ProtocolError,
};
use futures::future::Future;
use medea::conf::Server;
use medea::{
    api::client::server, conf::Conf, signalling::room_repo::RoomsRepository,
    start_static_rooms,
};
use medea_client_api_proto::{Command, Direction, Event, IceCandidate};
use serde_json::error::Error as SerdeError;
use std::rc::Rc;
use std::{
    cell::Cell,
    sync::{Arc, Mutex},
    time::Duration,
};

struct TestMember {
    writer: ClientWriter,
    events: Vec<Event>,
    test_fn: Box<FnMut(&Event)>,
}

impl Actor for TestMember {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb(ctx);
        ctx.run_later(Duration::new(5, 0), |act, _ctx| {
            panic!(
                "This test lasts more than 5 seconds. Most likely, this is \
                 not normal. Here is all events of member: {:?}",
                act.events
            );
        });
    }
}

impl TestMember {
    fn hb(&self, ctx: &mut Context<Self>) {
        ctx.run_later(Duration::new(1, 0), |act, ctx| {
            act.writer.text(r#"{"ping": 1}"#);
            act.hb(ctx);
        });
    }

    fn send_command(&mut self, msg: Command) {
        self.writer.text(&serde_json::to_string(&msg).unwrap());
    }

    pub fn start(uri: &str, test_fn: Box<FnMut(&Event)>) {
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

impl StreamHandler<WebMessage, ProtocolError> for TestMember {
    fn handle(&mut self, msg: WebMessage, _ctx: &mut Context<Self>) {
        match msg {
            WebMessage::Text(txt) => {
                let event: Result<Event, SerdeError> =
                    serde_json::from_str(&txt);
                if let Ok(event) = event {
                    self.events.push(event.clone());
                    (self.test_fn)(&event);
                    match event {
                        Event::PeerCreated {
                            peer_id,
                            sdp_offer,
                            tracks: _,
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

fn run_test_server(bind_port: u16, thread_name: &str) {
    let is_server_starting = Arc::new(Mutex::new(Cell::new(true)));
    let is_server_starting_ref = Arc::clone(&is_server_starting);
    let builder = std::thread::Builder::new().name(thread_name.to_string());

    let server_thread = builder
        .spawn(move || {
            dotenv::dotenv().ok();
            let _sys = System::new("medea");

            let config = Conf {
                server: Server {
                    static_specs_path: Some("tests/specs".to_string()),
                    bind_port,
                    ..Default::default()
                },
                ..Default::default()
            };

            match start_static_rooms(&config) {
                Ok(r) => {
                    let room_repo = RoomsRepository::new(r);
                    server::run(room_repo, config);
                }
                Err(e) => {
                    panic!("Server not started because of error: '{}'", e);
                }
            };
            let is_server_starting_guard =
                is_server_starting_ref.lock().unwrap();
            is_server_starting_guard.set(false);
        })
        .unwrap();

    // Wait until server is up
    while is_server_starting.lock().unwrap().get() {}

    server_thread.join().unwrap();
}

#[test]
fn should_work_pub_sub_video_call() {
    let bind_port = test_ports::SIGNALLING_TEST_PUB_SUB_TEST;
    run_test_server(bind_port, "should_work_pub_sub_video_call");
    let base_url = format!("ws://localhost:{}/ws", bind_port);

    let sys = System::new("medea-signaling-pub-sub-test");

    let mut events = Vec::new();
    let test_fn = move |event: &Event| {
        events.push(event.clone());
        if let Event::IceCandidateDiscovered {
            peer_id: _,
            candidate: _,
        } = event
        {
            let peers_count = events
                .iter()
                .filter(|e| match e {
                    Event::PeerCreated {
                        peer_id: _,
                        sdp_offer: _,
                        tracks: _,
                    } => true,
                    _ => false,
                })
                .count();
            assert_eq!(peers_count, 1);

            let mut is_caller = false;
            if let Event::PeerCreated {
                peer_id,
                sdp_offer,
                tracks,
            } = &events[0]
            {
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
                if let Event::SdpAnswerMade {
                    peer_id: _,
                    sdp_answer: _,
                } = &events[1]
                {
                    assert!(true);
                } else {
                    assert!(false);
                }

                if let Event::IceCandidateDiscovered {
                    peer_id: _,
                    candidate: _,
                } = &events[2]
                {
                    assert!(true);
                } else {
                    assert!(false);
                }
            } else {
                if let Event::IceCandidateDiscovered {
                    peer_id: _,
                    candidate: _,
                } = &events[1]
                {
                    assert!(true);
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
        &format!("{}/pub-sub-video-call/caller/test", base_url),
        Box::new(test_fn.clone()),
    );
    TestMember::start(
        &format!("{}/pub-sub-video-call/responder/test", base_url),
        Box::new(test_fn),
    );

    let _ = sys.run();
}

#[test]
fn should_work_three_members_p2p_video_call() {
    let bind_port = test_ports::SIGNALLING_TEST_THREE_MEMBERS;
    run_test_server(bind_port, "should_work_three_members_p2p_video_call");
    let base_url = format!("ws://localhost:{}/ws", bind_port);

    let sys = System::new("medea-signaling-3-members-test");

    let mut events = Vec::new();
    let mut peer_created_count = 0;
    let mut ice_candidates = 0;
    let members_tested = Rc::new(Cell::new(0));

    let test_fn = move |event: &Event| {
        events.push(event.clone());
        match event {
            Event::PeerCreated {
                peer_id: _,
                sdp_offer: _,
                tracks: _,
            } => {
                peer_created_count += 1;
            }
            Event::IceCandidateDiscovered {
                peer_id: _,
                candidate: _,
            } => {
                ice_candidates += 1;
                if ice_candidates == 2 {
                    assert_eq!(peer_created_count, 2);

                    events.iter().for_each(|e| match e {
                        Event::PeerCreated {
                            peer_id,
                            sdp_offer: _,
                            tracks,
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

                    members_tested.set(members_tested.get() + 1);
                    if members_tested.get() == 3 {
                        System::current().stop();
                    }
                }
            }
            _ => (),
        }
    };

    TestMember::start(
        &format!("{}/three-members-conference/caller/test", base_url),
        Box::new(test_fn.clone()),
    );
    TestMember::start(
        &format!("{}/three-members-conference/responder/test", base_url),
        Box::new(test_fn.clone()),
    );
    TestMember::start(
        &format!("{}/three-members-conference/responder2/test", base_url),
        Box::new(test_fn),
    );

    let _ = sys.run();
}

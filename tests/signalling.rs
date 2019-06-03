mod test_ports;

use actix::{Actor, Arbiter, AsyncContext, Context, StreamHandler, System};
use actix_web::ws::{
    Client, ClientWriter, Message as WebMessage, ProtocolError,
};
use futures::future::Future;
use medea::{
    api::client::server, conf::Conf, signalling::room_repo::RoomsRepository,
    start_static_rooms,
};
use medea_client_api_proto::{Command, Direction, Event, IceCandidate};
use serde_json::error::Error as SerdeError;
use std::{
    cell::Cell,
    sync::{Arc, Mutex},
    time::Duration,
};
use medea::conf::Server;

struct TestMember {
    writer: ClientWriter,
    is_caller: bool,
    events: Vec<Event>,
    test_fn: Box<Fn(&Vec<Event>)>,
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

    pub fn start(uri: &str, test_fn: Box<Fn(&Vec<Event>)>) {
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
                            is_caller: false,
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
                // println!("[{}]\t{}", self.is_caller, txt);
                let event: Result<Event, SerdeError> =
                    serde_json::from_str(&txt);
                if let Ok(event) = event {
                    self.events.push(event.clone());
                    match event {
                        Event::PeerCreated {
                            peer_id,
                            sdp_offer,
                            tracks: _,
                        } => {
                            match sdp_offer {
                                Some(_) => {
                                    self.is_caller = false;
                                    self.send_command(Command::MakeSdpAnswer {
                                        peer_id,
                                        sdp_answer: "responder_answer"
                                            .to_string(),
                                    });
                                }
                                None => {
                                    self.is_caller = true;
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
                        Event::IceCandidateDiscovered {
                            peer_id: _,
                            candidate: _,
                        } => {
                            (self.test_fn)(&self.events);
                        }
                        _ => (),
                    }
                }
            }
            _ => (),
        }
    }
}

// TODO: deal with async testing. Maybe use different ports?
fn run_test_server(bind_port: u16) {
    let is_server_starting = Arc::new(Mutex::new(Cell::new(true)));
    let is_server_starting_ref = Arc::clone(&is_server_starting);

    let server_thread = std::thread::spawn(move || {
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
        let is_server_starting_guard = is_server_starting_ref.lock().unwrap();
        is_server_starting_guard.set(false);
    });

    // Wait until server is up
    while is_server_starting.lock().unwrap().get() {}

    server_thread.join().unwrap();
}

#[test]
fn pub_sub_test() {
    let bind_port = test_ports::SIGNALLING_TEST_PUB_SUB_TEST;
    run_test_server(bind_port);
    let base_url = format!("ws://localhost:{}/ws", bind_port);

    let sys = System::new("medea-signalling-test");

    let pub_sub_test = |events: &Vec<Event>| {
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
    };

    TestMember::start(
        &format!("{}/pub-sub-video-call/caller/test", base_url),
        Box::new(pub_sub_test.clone()),
    );
    TestMember::start(
        &format!("{}/pub-sub-video-call/responder/test", base_url),
        Box::new(pub_sub_test.clone()),
    );

    let _ = sys.run();
}

// TODO: add ping-pong e2e test

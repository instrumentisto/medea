//! Signalling API e2e tests.

mod pub_sub_signallng;
mod three_pubs;

use std::time::Duration;

use actix::{
    Actor, Arbiter, AsyncContext, Context, Handler, Message, StreamHandler,
};
use actix_codec::Framed;
use actix_http::ws::{Codec, Message as WsMessage};
use awc::{
    error::WsProtocolError,
    ws::{CloseCode, CloseReason, Frame},
    BoxedSocket,
};
use futures::{future::Future, sink::Sink, stream::SplitSink, Stream};
use medea_client_api_proto::{Command, Direction, Event, IceCandidate};
use serde_json::error::Error as SerdeError;
use std::collections::HashMap;

/// Medea client for testing purposes.
pub struct TestMember {
    /// Writer to WebSocket.
    writer: SplitSink<Framed<BoxedSocket, Codec>>,

    /// All [`Event`]s which this [`TestMember`] received.
    /// This field used for give some debug info when test just stuck forever
    /// (most often, such a test will end on a timer of five seconds
    /// and display all events of this [`TestMember`]).
    events: Vec<Event>,

    /// Function which will be called at every received by this [`TestMember`]
    /// [`Event`].
    test_fn: Box<dyn FnMut(&Event, &mut Context<TestMember>)>,
}

impl TestMember {
    /// Signaling heartbeat for server.
    /// Most likely, this ping will never be sent,
    /// because it has been established that it is sent once per 3 seconds,
    /// and there are simply no tests that last so much.
    fn heartbeat(&self, ctx: &mut Context<Self>) {
        ctx.run_later(Duration::from_secs(3), |act, ctx| {
            act.writer
                .start_send(WsMessage::Text(r#"{"ping": 1}"#.to_string()))
                .unwrap();
            act.writer.poll_complete().unwrap();
            act.heartbeat(ctx);
        });
    }

    /// Send command to the server.
    fn send_command(&mut self, msg: Command) {
        // self.writer.text(&serde_json::to_string(&msg).unwrap());
        let json = serde_json::to_string(&msg).unwrap();
        self.writer.start_send(WsMessage::Text(json)).unwrap();
        self.writer.poll_complete().unwrap();
    }

    /// Start test member in new [`Arbiter`] by given URI.
    /// `test_fn` - is function which will be called at every [`Event`]
    /// received from server.
    pub fn start(
        uri: &str,
        test_fn: Box<dyn FnMut(&Event, &mut Context<TestMember>)>,
    ) {
        Arbiter::spawn(
            awc::Client::new()
                .ws(uri)
                .connect()
                .map_err(|e| panic!("Error: {}", e))
                .map(|(_, framed)| {
                    let (sink, stream) = framed.split();
                    TestMember::create(|ctx| {
                        TestMember::add_stream(stream, ctx);
                        TestMember {
                            writer: sink,
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

pub struct CloseSocket;

impl Message for CloseSocket {
    type Result = ();
}

impl Handler<CloseSocket> for TestMember {
    type Result = ();

    fn handle(&mut self, _: CloseSocket, _: &mut Self::Context) {
        self.writer
            .start_send(WsMessage::Close(Some(CloseReason {
                code: CloseCode::Normal,
                description: None,
            })))
            .unwrap();
        self.writer.poll_complete().unwrap();
    }
}

impl StreamHandler<Frame, WsProtocolError> for TestMember {
    /// Basic signalling implementation.
    /// A `TestMember::test_fn` [`FnMut`] function will be called for each
    /// [`Event`] received from test server.
    fn handle(&mut self, msg: Frame, ctx: &mut Context<Self>) {
        if let Frame::Text(txt) = msg {
            let txt = String::from_utf8(txt.unwrap().to_vec()).unwrap();
            let event: Result<Event, SerdeError> = serde_json::from_str(&txt);
            if let Ok(event) = event {
                self.events.push(event.clone());
                // Test function call
                (self.test_fn)(&event, ctx);

                if let Event::PeerCreated {
                    peer_id,
                    sdp_offer,
                    tracks,
                    ..
                } = event
                {
                    match sdp_offer {
                        Some(_) => self.send_command(Command::MakeSdpAnswer {
                            peer_id,
                            sdp_answer: "responder_answer".into(),
                        }),
                        None => self.send_command(Command::MakeSdpOffer {
                            peer_id,
                            sdp_offer: "caller_offer".into(),
                            mids: tracks
                                .into_iter()
                                .map(|t| t.id)
                                .enumerate()
                                .map(|(mid, id)| (id, mid.to_string()))
                                .collect(),
                        }),
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
            }
        }
    }
}

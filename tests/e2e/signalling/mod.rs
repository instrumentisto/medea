//! Signalling API E2E tests.

mod pub_sub_signallng;
mod three_pubs;

use std::time::Duration;

use actix::{
    Actor, Addr, Arbiter, AsyncContext, Context, Handler, StreamHandler,
};
use actix_codec::Framed;
use actix_http::ws;
use awc::{
    error::WsProtocolError,
    ws::{CloseCode, CloseReason, Frame},
    BoxedSocket,
};
use futures::{stream::SplitSink, Future, Future as _, Sink as _, Stream as _};
use medea_client_api_proto::{Command, Event, IceCandidate};
use serde_json::error::Error as SerdeError;

pub type MessageHandler =
    Box<dyn FnMut(&Event, &mut Context<TestMember>, Vec<&Event>)>;

/// Medea client for testing purposes.
pub struct TestMember {
    /// Writer to WebSocket.
    writer: SplitSink<Framed<BoxedSocket, ws::Codec>>,

    /// All [`Event`]s which this [`TestMember`] received.
    /// This field used for give some debug info when test just stuck forever
    /// (most often, such a test will end on a timer of five seconds
    /// and display all events of this [`TestMember`]).
    events: Vec<Event>,

    /// Max test lifetime, will panic when it will be exceeded.
    deadline: Option<Duration>,

    /// Function which will be called at every received [`Event`]
    /// by this [`TestMember`].
    on_message: MessageHandler,
}

impl TestMember {
    /// Starts signaling heartbeat for server.
    /// Most likely, this ping will never be sent,
    /// because it has been established that it is sent once per 3 seconds,
    /// and there are simply no tests that last so much.
    fn start_heartbeat(&self, ctx: &mut Context<Self>) {
        ctx.run_interval(Duration::from_secs(3), |act, _| {
            act.writer
                .start_send(ws::Message::Text(r#"{"ping": 1}"#.to_string()))
                .unwrap();
            act.writer.poll_complete().unwrap();
        });
    }

    /// Sends command to the server.
    fn send_command(&mut self, msg: &Command) {
        let json = serde_json::to_string(msg).unwrap();
        self.writer.start_send(ws::Message::Text(json)).ok();
        self.writer.poll_complete().ok();
    }

    /// Returns [`Future`] which will connect to the WebSocket and starts
    /// [`TestMember`] actor.
    pub fn connect(
        uri: &str,
        on_message: MessageHandler,
        deadline: Option<Duration>,
    ) -> impl Future<Item = Addr<Self>, Error = ()> {
        awc::Client::new()
            .ws(uri)
            .connect()
            .map_err(|e| panic!("Error: {}", e))
            .map(move |(_, framed)| {
                let (sink, stream) = framed.split();
                Self::create(move |ctx| {
                    Self::add_stream(stream, ctx);
                    Self {
                        writer: sink,
                        events: Vec::new(),
                        deadline,
                        on_message,
                    }
                })
            })
    }

    /// Starts test member in new [`Arbiter`] by given URI.
    /// `on_message` - is function which will be called at every [`Event`]
    /// received from server.
    pub fn start(
        uri: &str,
        on_message: MessageHandler,
        deadline: Option<Duration>,
    ) {
        Arbiter::spawn(Self::connect(uri, on_message, deadline).map(|_| ()))
    }
}

impl Actor for TestMember {
    type Context = Context<Self>;

    /// Starts heartbeat and sets a timer that will panic when 5 seconds will
    /// expire. The timer is needed because some tests may just stuck and listen
    /// socket forever.
    fn started(&mut self, ctx: &mut Self::Context) {
        self.start_heartbeat(ctx);

        if let Some(deadline) = self.deadline {
            ctx.run_later(deadline, |act, _ctx| {
                panic!(
                    "This test lasts more than 5 seconds. Most likely, this \
                     is not normal. Here are all events of member: {:?}",
                    act.events
                );
            });
        }
    }
}

#[derive(actix::Message)]
#[rtype(result = "()")]
pub struct CloseSocket;

impl Handler<CloseSocket> for TestMember {
    type Result = ();

    fn handle(&mut self, _: CloseSocket, _: &mut Self::Context) {
        self.writer
            .start_send(ws::Message::Close(Some(CloseReason {
                code: CloseCode::Normal,
                description: None,
            })))
            .unwrap();
        self.writer.poll_complete().unwrap();
    }
}

/// Basic signalling implementation.
/// [`TestMember::on_message`] function will be called for each [`Event`]
/// received from test server.
impl StreamHandler<Frame, WsProtocolError> for TestMember {
    fn handle(&mut self, msg: Frame, ctx: &mut Context<Self>) {
        if let Frame::Text(txt) = msg {
            let txt = String::from_utf8(txt.unwrap().to_vec()).unwrap();
            let event: Result<Event, SerdeError> = serde_json::from_str(&txt);
            if let Ok(event) = event {
                // Test function call

                if let Event::PeerCreated {
                    peer_id,
                    sdp_offer,
                    tracks,
                    ..
                } = &event
                {
                    match sdp_offer {
                        Some(_) => self.send_command(&Command::MakeSdpAnswer {
                            peer_id: *peer_id,
                            sdp_answer: "responder_answer".into(),
                        }),
                        None => self.send_command(&Command::MakeSdpOffer {
                            peer_id: *peer_id,
                            sdp_offer: "caller_offer".into(),
                            mids: tracks
                                .iter()
                                .map(|t| t.id)
                                .enumerate()
                                .map(|(mid, id)| (id, mid.to_string()))
                                .collect(),
                        }),
                    }

                    self.send_command(&Command::SetIceCandidate {
                        peer_id: *peer_id,
                        candidate: IceCandidate {
                            candidate: "ice_candidate".to_string(),
                            sdp_m_line_index: None,
                            sdp_mid: None,
                        },
                    });
                }
                let mut events: Vec<&Event> = self.events.iter().collect();
                events.push(&event);
                (self.on_message)(&event, ctx, events);
                self.events.push(event);
            }
        }
    }
}

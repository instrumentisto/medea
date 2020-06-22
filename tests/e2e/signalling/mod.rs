//! Signalling API E2E tests.

mod command_validation;
mod ice_restart;
mod pub_sub_signallng;
mod rpc_settings;
mod three_pubs;

use std::{collections::HashMap, time::Duration};

use actix::{
    Actor, ActorContext, Addr, Arbiter, AsyncContext, Context, Handler,
    StreamHandler,
};
use actix_codec::Framed;
use actix_http::ws;
use awc::{
    error::WsProtocolError,
    ws::{CloseCode, CloseReason, Frame},
    BoxedSocket,
};
use futures::{executor, stream::SplitSink, SinkExt as _, StreamExt as _};
use medea_client_api_proto::{
    ClientMsg, Command, Event, IceCandidate, PeerId, RpcSettings, ServerMsg,
};

pub type MessageHandler =
    Box<dyn FnMut(&Event, &mut Context<TestMember>, Vec<&Event>)>;

pub type ConnectionEventHandler = Box<dyn FnMut(ConnectionEvent)>;

/// Event which will be provided into [`ConnectionEventHandler`] when connection
/// will be established or disconnected.
pub enum ConnectionEvent {
    /// Connection established.
    Started,

    /// [`RpcSettings`] [`ServerMsg`] received.
    SettingsReceived(RpcSettings),

    /// Connection disconnected.
    Stopped,
}

/// Medea client for testing purposes.
pub struct TestMember {
    /// Writer to WebSocket.
    sink: SplitSink<Framed<BoxedSocket, ws::Codec>, ws::Message>,

    /// All [`Event`]s which this [`TestMember`] received.
    /// This field used for give some debug info when test just stuck forever
    /// (most often, such a test will end on a timer of five seconds
    /// and display all events of this [`TestMember`]).
    events: Vec<Event>,

    /// List of peers created on this client.
    known_peers: Vec<PeerId>,

    /// Max test lifetime, will panic when it will be exceeded.
    deadline: Option<Duration>,

    /// Function which will be called at every received [`Event`]
    /// by this [`TestMember`].
    on_message: Option<MessageHandler>,

    /// Function which will be called when connection will be established and
    /// disconnected.
    on_connection_event: Option<ConnectionEventHandler>,
}

impl TestMember {
    /// Sends command to the server.
    fn send_command(&mut self, msg: Command) {
        executor::block_on(async move {
            let json = serde_json::to_string(&ClientMsg::Command(msg)).unwrap();
            self.sink.send(ws::Message::Text(json)).await.unwrap();
            self.sink.flush().await.unwrap();
        });
    }

    /// Sends pong to the server.
    fn send_pong(&mut self, id: u64) {
        executor::block_on(async move {
            let json = serde_json::to_string(&ClientMsg::Pong(id)).unwrap();
            self.sink.send(ws::Message::Text(json)).await.unwrap();
            self.sink.flush().await.unwrap();
        });
    }

    /// Returns [`Future`] which will connect to the WebSocket and starts
    /// [`TestMember`] actor.
    pub async fn connect(
        uri: &str,
        on_message: Option<MessageHandler>,
        on_connection_event: Option<ConnectionEventHandler>,
        deadline: Option<Duration>,
    ) -> Addr<Self> {
        let (_, framed) = awc::Client::new().ws(uri).connect().await.unwrap();

        let (sink, stream) = framed.split();

        Self::create(move |ctx| {
            Self::add_stream(stream, ctx);
            Self {
                sink,
                events: Vec::new(),
                known_peers: Vec::new(),
                deadline,
                on_message,
                on_connection_event,
            }
        })
    }

    /// Starts test member on current thread by given URI.
    ///
    /// `on_message` - is function which will be called at every [`Event`]
    /// received from server.
    ///
    /// `on_connection_event` - is function which will be called when connection
    /// will be established and disconnected.
    pub fn start(
        uri: String,
        on_message: Option<MessageHandler>,
        on_connection_event: Option<ConnectionEventHandler>,
        deadline: Option<Duration>,
    ) {
        Arbiter::spawn(async move {
            Self::connect(&uri, on_message, on_connection_event, deadline)
                .await;
        })
    }
}

impl Actor for TestMember {
    type Context = Context<Self>;

    /// Starts heartbeat and sets a timer that will panic when 5 seconds will
    /// expire. The timer is needed because some tests may just stuck and listen
    /// socket forever.
    fn started(&mut self, ctx: &mut Self::Context) {
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
pub struct CloseSocket(pub CloseCode);

impl Handler<CloseSocket> for TestMember {
    type Result = ();

    fn handle(&mut self, msg: CloseSocket, _: &mut Self::Context) {
        executor::block_on(async move {
            self.sink
                .send(ws::Message::Close(Some(CloseReason {
                    code: msg.0,
                    description: None,
                })))
                .await
                .unwrap();
            self.sink.flush().await.unwrap();
            self.sink.close().await.unwrap();
        });
    }
}

#[derive(actix::Message)]
#[rtype(result = "()")]
pub struct SendCommand(pub Command);

impl Handler<SendCommand> for TestMember {
    type Result = ();

    fn handle(&mut self, msg: SendCommand, _: &mut Self::Context) {
        self.send_command(msg.0);
    }
}

/// Basic signalling implementation.
/// [`TestMember::on_message`] function will be called for each [`Event`]
/// received from test server.
impl StreamHandler<Result<Frame, WsProtocolError>> for TestMember {
    fn handle(
        &mut self,
        msg: Result<Frame, WsProtocolError>,
        ctx: &mut Context<Self>,
    ) {
        if let Frame::Text(txt) = msg.unwrap() {
            let txt = String::from_utf8(txt.to_vec()).unwrap();
            let server_msg: ServerMsg = serde_json::from_str(&txt).unwrap();

            match server_msg {
                ServerMsg::Ping(id) => self.send_pong(id),
                ServerMsg::Event(event) => {
                    match &event {
                        Event::PeerCreated {
                            peer_id,
                            sdp_offer,
                            tracks,
                            ..
                        } => {
                            self.known_peers.push(*peer_id);
                            match sdp_offer {
                                Some(_) => {
                                    self.send_command(Command::MakeSdpAnswer {
                                        peer_id: *peer_id,
                                        sdp_answer: "responder_answer".into(),
                                        senders_statuses: HashMap::new(),
                                    })
                                }
                                None => {
                                    self.send_command(Command::MakeSdpOffer {
                                        peer_id: *peer_id,
                                        sdp_offer: "caller_offer".into(),
                                        mids: tracks
                                            .iter()
                                            .map(|t| t.id)
                                            .enumerate()
                                            .map(|(mid, id)| {
                                                (id, mid.to_string())
                                            })
                                            .collect(),
                                        senders_statuses: HashMap::new(),
                                    })
                                }
                            };

                            self.send_command(Command::SetIceCandidate {
                                peer_id: *peer_id,
                                candidate: IceCandidate {
                                    candidate: "ice_candidate".to_string(),
                                    sdp_m_line_index: None,
                                    sdp_mid: None,
                                },
                            });
                        }
                        Event::SdpAnswerMade { peer_id, .. }
                        | Event::IceCandidateDiscovered { peer_id, .. }
                        | Event::TracksUpdated { peer_id, .. }
                        | Event::SdpOfferMade { peer_id, .. }
                        | Event::RenegotiationStarted { peer_id, .. } => {
                            assert!(self.known_peers.contains(peer_id))
                        }
                        Event::PeersRemoved { .. } => {}
                    }
                    let mut events: Vec<&Event> = self.events.iter().collect();
                    events.push(&event);
                    if let Some(func) = self.on_message.as_mut() {
                        func(&event, ctx, events);
                    }
                    self.events.push(event);
                }
                ServerMsg::RpcSettings(settings) => {
                    if let Some(func) = self.on_connection_event.as_mut() {
                        func(ConnectionEvent::SettingsReceived(settings))
                    };
                }
            }
        }
    }

    fn started(&mut self, _: &mut Self::Context) {
        if let Some(func) = self.on_connection_event.as_mut() {
            func(ConnectionEvent::Started)
        };
    }

    fn finished(&mut self, ctx: &mut Self::Context) {
        if let Some(func) = self.on_connection_event.as_mut() {
            func(ConnectionEvent::Stopped)
        };

        ctx.stop()
    }
}

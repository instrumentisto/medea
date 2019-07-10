//! Room definitions and implementations. Room is responsible for media
//! connection establishment between concrete [`Member`]s.

use std::time::Duration;

use actix::{
    fut::wrap_future, Actor, ActorFuture, AsyncContext, Context, Handler,
    Message,
};
use failure::Fail;
use futures::{future, Future};
use hashbrown::HashMap;

use medea_client_api_proto::{Command, Event, IceCandidate};

use crate::{
    api::{
        client::rpc_connection::{
            AuthorizationError, Authorize, CommandMessage, RpcConnectionClosed,
            RpcConnectionEstablished,
        },
        control::{Member, MemberId},
    },
    graceful_shutdown,
    log::prelude::*,
    media::{
        New, Peer, PeerId, PeerStateError, PeerStateMachine,
        WaitLocalHaveRemote, WaitLocalSdp, WaitRemoteSdp,
    },
    signalling::{participants::ParticipantService, peers::PeerRepository},
    turn::TurnAuthService,
    utils::graceful_shutdown::ShutdownMessage,
};

/// ID of [`Room`].
pub type Id = u64;

/// Ergonomic type alias for using [`ActorFuture`] for [`Room`].
pub type ActFuture<I, E> =
    Box<dyn ActorFuture<Actor = Room, Item = I, Error = E>>;

#[derive(Debug, Fail)]
#[allow(clippy::module_name_repetitions)]
pub enum RoomError {
    #[fail(display = "Couldn't find Peer with [id = {}]", _0)]
    PeerNotFound(PeerId),
    #[fail(display = "Couldn't find Member with [id = {}]", _0)]
    MemberNotFound(MemberId),
    #[fail(display = "Member [id = {}] does not have Turn credentials", _0)]
    NoTurnCredentials(MemberId),
    #[fail(display = "Couldn't find RpcConnection with Member [id = {}]", _0)]
    ConnectionNotExists(MemberId),
    #[fail(display = "Unable to send event to Member [id = {}]", _0)]
    UnableToSendEvent(MemberId),
    #[fail(display = "PeerError: {}", _0)]
    PeerStateError(PeerStateError),
    #[fail(display = "Generic room error: {}", _0)]
    BadRoomSpec(String),
    #[fail(display = "Turn service error: {}", _0)]
    TurnServiceError(String),
}

impl From<PeerStateError> for RoomError {
    fn from(err: PeerStateError) -> Self {
        RoomError::PeerStateError(err)
    }
}

#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
pub enum RoomState {
    Started,
    Stopping,
    Stopped,
}

/// Media server room with its [`Member`]s.
#[derive(Debug)]
pub struct Room {
    id: Id,

    /// [`RpcConnection`]s of [`Member`]s in this [`Room`].
    pub participants: ParticipantService,

    /// [`Peer`]s of [`Member`]s in this [`Room`].
    peers: PeerRepository,

    /// Current [`Room`]'s state
    state: RoomState,
}

impl Room {
    /// Create new instance of [`Room`].
    pub fn new(
        id: Id,
        members: HashMap<MemberId, Member>,
        peers: HashMap<PeerId, PeerStateMachine>,
        reconnect_timeout: Duration,
        turn: Box<dyn TurnAuthService>,
    ) -> Self {
        Self {
            id,
            peers: PeerRepository::from(peers),
            participants: ParticipantService::new(
                id,
                members,
                turn,
                reconnect_timeout,
            ),
            state: RoomState::Started,
        }
    }

    /// Sends [`Event::PeerCreated`] to one of specified [`Peer`]s based on
    /// which of them has any outbound tracks. That [`Peer`] state will be
    /// changed to [`WaitLocalSdp`] state. Both provided peers must be in
    /// [`New`] state. At least one of provided peers must have outbound
    /// tracks.
    fn send_peer_created(
        &mut self,
        peer1_id: PeerId,
        peer2_id: PeerId,
    ) -> Result<ActFuture<(), RoomError>, RoomError> {
        let peer1: Peer<New> = self.peers.take_inner_peer(peer1_id)?;
        let peer2: Peer<New> = self.peers.take_inner_peer(peer2_id)?;

        // decide which peer is sender
        let (sender, receiver) = if peer1.is_sender() {
            (peer1, peer2)
        } else if peer2.is_sender() {
            (peer2, peer1)
        } else {
            self.peers.add_peer(peer1);
            self.peers.add_peer(peer2);
            return Err(RoomError::BadRoomSpec(format!(
                "Error while trying to connect Peer [id = {}] and Peer [id = \
                 {}] cause neither of peers are senders",
                peer1_id, peer2_id
            )));
        };
        self.peers.add_peer(receiver);

        let sender = sender.start();
        let member_id = sender.member_id();
        let ice_servers = self
            .participants
            .get_member(member_id)
            .ok_or_else(|| RoomError::MemberNotFound(member_id))?
            .ice_user
            .as_ref()
            .ok_or_else(|| RoomError::NoTurnCredentials(member_id))?
            .servers_list();
        let peer_created = Event::PeerCreated {
            peer_id: sender.id(),
            sdp_offer: None,
            tracks: sender.tracks(),
            ice_servers,
        };
        self.peers.add_peer(sender);
        Ok(Box::new(wrap_future(
            self.participants
                .send_event_to_member(member_id, peer_created),
        )))
    }

    /// Sends [`Event::PeerCreated`] to provided [`Peer`] partner. Provided
    /// [`Peer`] state must be [`WaitLocalSdp`] and will be changed to
    /// [`WaitRemoteSdp`], partners [`Peer`] state must be [`New`] and will be
    /// changed to [`WaitLocalHaveRemote`].
    fn handle_make_sdp_offer(
        &mut self,
        from_peer_id: PeerId,
        sdp_offer: String,
    ) -> Result<ActFuture<(), RoomError>, RoomError> {
        let from_peer: Peer<WaitLocalSdp> =
            self.peers.take_inner_peer(from_peer_id)?;
        let to_peer_id = from_peer.partner_peer_id();
        let to_peer: Peer<New> = self.peers.take_inner_peer(to_peer_id)?;

        let from_peer = from_peer.set_local_sdp(sdp_offer.clone());
        let to_peer = to_peer.set_remote_sdp(sdp_offer.clone());

        let to_member_id = to_peer.member_id();
        let ice_servers = self
            .participants
            .get_member(to_member_id)
            .ok_or_else(|| RoomError::MemberNotFound(to_member_id))?
            .ice_user
            .as_ref()
            .ok_or_else(|| RoomError::NoTurnCredentials(to_member_id))?
            .servers_list();

        let event = Event::PeerCreated {
            peer_id: to_peer_id,
            sdp_offer: Some(sdp_offer),
            tracks: to_peer.tracks(),
            ice_servers,
        };

        self.peers.add_peer(from_peer);
        self.peers.add_peer(to_peer);

        Ok(Box::new(wrap_future(
            self.participants.send_event_to_member(to_member_id, event),
        )))
    }

    /// Sends [`Event::SdpAnswerMade`] to provided [`Peer`] partner. Provided
    /// [`Peer`] state must be [`WaitLocalHaveRemote`] and will be changed to
    /// [`Stable`], partners [`Peer`] state must be [`WaitRemoteSdp`] and will
    /// be changed to [`Stable`].
    fn handle_make_sdp_answer(
        &mut self,
        from_peer_id: PeerId,
        sdp_answer: String,
    ) -> Result<ActFuture<(), RoomError>, RoomError> {
        let from_peer: Peer<WaitLocalHaveRemote> =
            self.peers.take_inner_peer(from_peer_id)?;
        let to_peer_id = from_peer.partner_peer_id();
        let to_peer: Peer<WaitRemoteSdp> =
            self.peers.take_inner_peer(to_peer_id)?;

        let from_peer = from_peer.set_local_sdp(sdp_answer.clone());
        let to_peer = to_peer.set_remote_sdp(&sdp_answer);

        let to_member_id = to_peer.member_id();
        let event = Event::SdpAnswerMade {
            peer_id: to_peer_id,
            sdp_answer,
        };

        self.peers.add_peer(from_peer);
        self.peers.add_peer(to_peer);

        Ok(Box::new(wrap_future(
            self.participants.send_event_to_member(to_member_id, event),
        )))
    }

    /// Sends [`Event::IceCandidateDiscovered`] to provided [`Peer`] partner.
    /// Both [`Peer`]s may have any state except [`New`].
    fn handle_set_ice_candidate(
        &mut self,
        from_peer_id: PeerId,
        candidate: IceCandidate,
    ) -> Result<ActFuture<(), RoomError>, RoomError> {
        let from_peer = self.peers.get_peer(from_peer_id)?;
        if let PeerStateMachine::New(_) = from_peer {
            return Err(RoomError::PeerStateError(PeerStateError::WrongState(
                from_peer_id,
                "Not New",
                format!("{}", from_peer),
            )));
        }

        let to_peer_id = from_peer.partner_peer_id();
        let to_peer = self.peers.get_peer(to_peer_id)?;
        if let PeerStateMachine::New(_) = to_peer {
            return Err(RoomError::PeerStateError(PeerStateError::WrongState(
                to_peer_id,
                "Not New",
                format!("{}", to_peer),
            )));
        }

        let to_member_id = to_peer.member_id();
        let event = Event::IceCandidateDiscovered {
            peer_id: to_peer_id,
            candidate,
        };

        Ok(Box::new(wrap_future(
            self.participants.send_event_to_member(to_member_id, event),
        )))
    }
}

/// [`Actor`] implementation that provides an ergonomic way
/// to interact with [`Room`].
impl Actor for Room {
    type Context = Context<Self>;
}

impl Handler<Authorize> for Room {
    type Result = Result<(), AuthorizationError>;

    /// Responses with `Ok` if `RpcConnection` is authorized, otherwise `Err`s.
    fn handle(
        &mut self,
        msg: Authorize,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        self.participants
            .get_member_by_id_and_credentials(msg.member_id, &msg.credentials)
            .map(|_| ())
    }
}

/// Signal of start signaling between specified [`Peer`]'s.
#[derive(Debug, Message)]
#[rtype(result = "Result<(), ()>")]
pub struct ConnectPeers(PeerId, PeerId);

impl Handler<ConnectPeers> for Room {
    type Result = ActFuture<(), ()>;

    /// Check state of interconnected [`Peer`]s and sends [`Event`] about
    /// [`Peer`] created to remote [`Member`].
    fn handle(
        &mut self,
        msg: ConnectPeers,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        match self.send_peer_created(msg.0, msg.1) {
            Ok(res) => {
                Box::new(res.map_err(|err, _, ctx: &mut Context<Self>| {
                    error!(
                        "Failed handle command, because {}. Room will be \
                         stopped.",
                        err
                    );
                    ctx.spawn(
                        wrap_future(
                            ctx.address()
                                .send(CloseRoom {})
                                .map_err(|_| ())
                                .and_then(std::result::Result::unwrap)
                        )
                            .map(|_, _, _| ())
                            .map_err(|_, _, _| ()),
                    );
                }))
            }
            Err(err) => {
                error!(
                    "Failed handle command, because {}. Room will be stopped.",
                    err
                );
                ctx.spawn(
                    wrap_future(
                        ctx.address()
                            .send(CloseRoom {})
                            .map_err(|_| ())
                            .and_then(std::result::Result::unwrap)
                    )
                        .map(|_, _, _| ())
                        .map_err(|_, _, _| ()),
                );
                Box::new(wrap_future(future::ok(())))
            }
        }
    }
}

impl Handler<CommandMessage> for Room {
    type Result = ActFuture<(), ()>;

    /// Receives [`Command`] from Web client and passes it to corresponding
    /// handlers. Will emit [`CloseRoom`] on any error.
    fn handle(
        &mut self,
        msg: CommandMessage,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        let result = match msg.into() {
            Command::MakeSdpOffer { peer_id, sdp_offer } => {
                self.handle_make_sdp_offer(peer_id, sdp_offer)
            }
            Command::MakeSdpAnswer {
                peer_id,
                sdp_answer,
            } => self.handle_make_sdp_answer(peer_id, sdp_answer),
            Command::SetIceCandidate { peer_id, candidate } => {
                self.handle_set_ice_candidate(peer_id, candidate)
            }
        };

        match result {
            Ok(res) => {
                Box::new(res.map_err(|err, _, ctx: &mut Context<Self>| {
                    error!(
                        "Failed handle command, because {}. Room will be \
                         stopped.",
                        err
                    );
                    ctx.spawn(
                        wrap_future(
                            ctx.address()
                                .send(CloseRoom {})
                                .map_err(|_| ())
                                .and_then(std::result::Result::unwrap)
                        )
                            .map(|_, _, _| ())
                            .map_err(|_, _, _| ()),
                    );
                }))
            }
            Err(err) => {
                error!(
                    "Failed handle command, because {}. Room will be stopped.",
                    err
                );
                ctx.spawn(
                    wrap_future(
                        ctx.address()
                            .send(CloseRoom {})
                            .map_err(|_| ())
                            .and_then(std::result::Result::unwrap)
                    )
                        .map(|_, _, _| ())
                        .map_err(|_, _, _| ()),
                );
                Box::new(wrap_future(future::ok(())))
            }
        }
    }
}

impl Handler<RpcConnectionEstablished> for Room {
    type Result = ActFuture<(), ()>;

    /// Saves new [`RpcConnection`] in [`ParticipantService`], initiates media
    /// establishment between members.
    fn handle(
        &mut self,
        msg: RpcConnectionEstablished,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        let member_id = msg.member_id;
        info!("RpcConnectionEstablished for member {}", msg.member_id);

        let fut =
            self.participants
                .connection_established(ctx, msg.member_id, msg.connection)
                .map_err(|err, _, _| {
                    error!("RpcConnectionEstablished error {:?}", err)
                })
                .map(move |_, room, ctx| {
                    room.peers
                        .get_peers_by_member_id(member_id)
                        .into_iter()
                        .for_each(|peer| {
                            if let PeerStateMachine::New(peer) = peer {
                                if room.participants.member_has_connection(
                                    peer.partner_member_id(),
                                ) {
                                    ctx.notify(ConnectPeers(
                                        peer.id(),
                                        peer.partner_peer_id(),
                                    ));
                                }
                            }
                        });
                });
        Box::new(fut)
    }
}

/// Signal of close [`Room`].
#[derive(Debug, Message)]
#[rtype(result = "Result<Box<(dyn Future<Item = (), Error = ()> + Send)>, ()>")]
#[allow(clippy::module_name_repetitions)]
pub struct CloseRoom {}

impl Handler<CloseRoom> for Room {
    type Result = Result<Box<(dyn Future<Item = (), Error = ()> + Send)>, ()>;

    /// Sends to remote [`Member`] the [`Event`] about [`Peer`] removed.
    /// Closes all active [`RpcConnection`]s.
    fn handle(
        &mut self,
        _msg: CloseRoom,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        info!("Closing Room [id = {:?}]", self.id);
        self.state = RoomState::Stopping;

        let room_recipient = ctx.address().recipient();
        let drop_fut = self.participants.drop_connections(ctx)
            .then(move |_| {
                match room_recipient.try_send(RoomClosed{}) {
                    Ok(_) => (),
                    Err(e) => error!("Cannot change Room's state to closed: {:?}", e),
                };
                futures::future::ok(())
            });

        Ok(Box::new(drop_fut))
    }
}

// Close room on `SIGINT`, `SIGTERM`, `SIGQUIT`, `SIGHUP` signals.
impl Handler<ShutdownMessage> for Room {
    type Result = graceful_shutdown::ShutdownMessageResult;

    fn handle(
        &mut self,
        _: ShutdownMessage,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        info!("Shutting down Room: {:?}", self.id);

        let room_id = self.id;
        Ok(Box::new(
            ctx.address()
                .send(CloseRoom {})
                .map_err(|_| ())
                .and_then(std::result::Result::unwrap)
                .map(|_| ())
                .map_err(move |_| {
                    error!("Error closing room {:?}", room_id);
                }),
        ))
    }
}

/// Signal to change [`Room`]'s state to closed.
#[derive(Message)]
#[rtype(result = "()")]
#[allow(clippy::module_name_repetitions)]
struct RoomClosed {}

impl Handler<RoomClosed> for Room {
    type Result = ();

    fn handle(
        &mut self,
        _msg: RoomClosed,
        _: &mut Self::Context,
    ) { self.state = RoomState::Stopped; }
}

impl Handler<RpcConnectionClosed> for Room {
    type Result = ();

    /// Passes message to [`ParticipantService`] to cleanup stored connections.
    fn handle(&mut self, msg: RpcConnectionClosed, ctx: &mut Self::Context) {
        info!(
            "RpcConnectionClosed for member {}, reason {:?}",
            msg.member_id, msg.reason
        );

        self.participants
            .connection_closed(ctx, msg.member_id, &msg.reason);
    }
}

#[cfg(test)]
mod test {
    use std::sync::{atomic::AtomicUsize, Arc, Mutex};

    use actix::{Addr, System};

    use medea_client_api_proto::{
        AudioSettings, Direction, IceServer, MediaType, Track, VideoSettings,
    };

    use crate::{
        api::client::rpc_connection::test::TestConnection, media::create_peers,
        turn::new_turn_auth_service_mock,
    };

    use super::*;

    fn start_room() -> Addr<Room> {
        let members = hashmap! {
            1 => Member{
                id: 1,
                credentials: "caller_credentials".to_owned(),
                ice_user: None
            },
            2 => Member{
                id: 2,
                credentials: "responder_credentials".to_owned(),
                ice_user: None
            },
        };
        Room::new(
            1,
            members,
            create_peers(1, 2),
            Duration::from_secs(10),
            new_turn_auth_service_mock(),
        )
        .start()
    }

    #[test]
    fn start_signaling() {
        let stopped = Arc::new(AtomicUsize::new(0));
        let caller_events = Arc::new(Mutex::new(vec![]));
        let caller_events_clone = Arc::clone(&caller_events);
        let responder_events = Arc::new(Mutex::new(vec![]));
        let responder_events_clone = Arc::clone(&responder_events);

        System::run(move || {
            let room = start_room();
            let room_clone = room.clone();
            let stopped_clone = stopped.clone();

            TestConnection {
                events: caller_events_clone,
                member_id: 1,
                room: room_clone,
                stopped: stopped_clone,
            }
            .start();

            TestConnection {
                events: responder_events_clone,
                member_id: 2,
                room,
                stopped,
            }
            .start();
        })
        .unwrap();

        let caller_events = caller_events.lock().unwrap();
        let responder_events = responder_events.lock().unwrap();
        assert_eq!(
            caller_events.to_vec(),
            vec![
                serde_json::to_string(&Event::PeerCreated {
                    peer_id: 1,
                    sdp_offer: None,
                    tracks: vec![
                        Track {
                            id: 1,
                            direction: Direction::Send { receivers: vec![2] },
                            media_type: MediaType::Audio(AudioSettings {}),
                        },
                        Track {
                            id: 2,
                            direction: Direction::Send { receivers: vec![2] },
                            media_type: MediaType::Video(VideoSettings {}),
                        },
                    ],
                    ice_servers: vec![
                        IceServer {
                            urls: vec!["stun:5.5.5.5:1234".to_string()],
                            username: None,
                            credential: None,
                        },
                        IceServer {
                            urls: vec![
                                "turn:5.5.5.5:1234".to_string(),
                                "turn:5.5.5.5:1234?transport=tcp".to_string()
                            ],
                            username: Some("username".to_string()),
                            credential: Some("password".to_string()),
                        },
                    ],
                })
                .unwrap(),
                serde_json::to_string(&Event::SdpAnswerMade {
                    peer_id: 1,
                    sdp_answer: "responder_answer".into(),
                })
                .unwrap(),
                serde_json::to_string(&Event::IceCandidateDiscovered {
                    peer_id: 1,
                    candidate: IceCandidate {
                        candidate: "ice_candidate".to_owned(),
                        sdp_m_line_index: None,
                        sdp_mid: None
                    },
                })
                .unwrap(),
            ]
        );

        assert_eq!(
            responder_events.to_vec(),
            vec![
                serde_json::to_string(&Event::PeerCreated {
                    peer_id: 2,
                    sdp_offer: Some("caller_offer".into()),
                    tracks: vec![
                        Track {
                            id: 1,
                            direction: Direction::Recv { sender: 1 },
                            media_type: MediaType::Audio(AudioSettings {}),
                        },
                        Track {
                            id: 2,
                            direction: Direction::Recv { sender: 1 },
                            media_type: MediaType::Video(VideoSettings {}),
                        },
                    ],
                    ice_servers: vec![
                        IceServer {
                            urls: vec!["stun:5.5.5.5:1234".to_string()],
                            username: None,
                            credential: None,
                        },
                        IceServer {
                            urls: vec![
                                "turn:5.5.5.5:1234".to_string(),
                                "turn:5.5.5.5:1234?transport=tcp".to_string()
                            ],
                            username: Some("username".to_string()),
                            credential: Some("password".to_string()),
                        },
                    ],
                })
                .unwrap(),
                serde_json::to_string(&Event::IceCandidateDiscovered {
                    peer_id: 2,
                    candidate: IceCandidate {
                        candidate: "ice_candidate".to_owned(),
                        sdp_m_line_index: None,
                        sdp_mid: None
                    },
                })
                .unwrap(),
            ]
        );
    }
}

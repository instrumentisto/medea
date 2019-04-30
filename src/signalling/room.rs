//! Room definitions and implementations.

use std::time::{Duration, Instant};

use actix::{
    fut::wrap_future, Actor, ActorFuture, AsyncContext, Context, Handler,
    Message, SpawnHandle,
};
use failure::Fail;
use futures::{
    future::{self, join_all, Either},
    Future,
};
use hashbrown::HashMap;

use crate::{
    api::{
        client::rpc_connection::{
            AuthorizationError, Authorize, ClosedReason, RpcConnection,
            RpcConnectionClosed, RpcConnectionEstablished,
        },
        control::{Id as MemberId, Member},
        protocol::{Command, Event},
    },
    log::prelude::*,
    media::peer::{Id as PeerId, PeerStateMachine},
    signalling::peer_repo::PeerRepository
};

#[derive(Fail, Debug)]
#[allow(clippy::module_name_repetitions)]
pub enum RoomError {
    #[fail(display = "Unknown peer {}", _0)]
    UnknownPeer(PeerId),
    #[fail(display = "Unmatched states between peers {} and {}", _0, _1)]
    UnmatchedState(PeerId, PeerId),
    #[fail(display = "Member {} not connected at moment", _0)]
    ConnectionNotExists(MemberId),
    #[fail(display = "Unable send event to member {}", _0)]
    UnableSendEvent(MemberId),
}

/// ID of [`Room`].
pub type Id = u64;

/// Media server room with its [`Member`]s.
#[derive(Debug)]
pub struct Room {
    id: Id,

    /// [`Member`]s which currently are present in this [`Room`].
    members: HashMap<MemberId, Member>,

    /// Established [`RpcConnection`]s of [`Member`]s in this [`Room`].
    // TODO: Replace Box<dyn RpcConnection>> with enum,
    //       as the set of all possible RpcConnection types is not closed.
    pub connections: HashMap<MemberId, Box<dyn RpcConnection>>,

    /// [`Peer`]s of [`Member`]s in this [`Room`].
    peers: PeerRepository,

    /// Timeout for close [`RpcConnection`] after receiving
    /// [`RpcConnectionClosed`] message.
    reconnect_timeout: Duration,

    /// Stores [`RpcConnection`] drop tasks.
    /// If [`RpcConnection`] is lost, ['Room'] waits for connection_timeout
    /// before dropping it irrevocably in case it gets reestablished.
    no_reconnect_tasks: HashMap<MemberId, SpawnHandle>,
}

impl Room {
    /// Create new instance of [`Room`].
    pub fn new(
        id: Id,
        members: HashMap<MemberId, Member>,
        peers: HashMap<PeerId, PeerStateMachine>,
        connection_timeout: Duration,
    ) -> Self {
        Self {
            id,
            connections: HashMap::new(),
            no_reconnect_tasks: HashMap::new(),
            members,
            peers:PeerRepository::from(peers),
            reconnect_timeout: connection_timeout,
        }
    }

    /// Send [`Event`] to specified remote [`Member`].
    fn send_event_to_member(
        &mut self,
        member_id: MemberId,
        event: Event,
    ) -> impl Future<Item = (), Error = RoomError> {
        match self.connections.get(&member_id) {
            Some(conn) => Either::A(
                conn.send_event(event)
                    .map_err(move |_| RoomError::UnableSendEvent(member_id)),
            ),
            None => Either::B(future::err(RoomError::ConnectionNotExists(
                member_id,
            ))),
        }
    }

    /// Check state of the specified and associated [`Peer`].
    /// Returns [`Event`] to caller that [`Peer`] is created.
    fn handle_peer_created(
        &mut self,
        from_peer_id: PeerId,
        to_peer_id: PeerId,
    ) -> Result<(MemberId, Event), RoomError> {
        let from_peer = self.peers.take_peer(from_peer_id)?;
        let to_peer = self.peers.take_peer(to_peer_id)?;

        let (from_peer, to_peer) = match (from_peer, to_peer) {
            (
                PeerStateMachine::New(peer_from),
                PeerStateMachine::New(peer_to),
            ) => Ok((peer_from, peer_to)),
            (from_peer, to_peer) => {
                self.peers.add_peer(from_peer_id, from_peer);
                self.peers.add_peer(to_peer_id, to_peer);
                Err(RoomError::UnmatchedState(from_peer_id, to_peer_id))
            }
        }?;

        let to_peer = to_peer.start();
        let to_member_id = to_peer.member_id();

        let event = Event::PeerCreated {
            peer_id: to_peer_id,
            sdp_offer: None,
            tracks: to_peer.tracks(),
        };

        self.peers.add_peer(from_peer_id, PeerStateMachine::New(from_peer));
        self.peers.add_peer(to_peer_id, PeerStateMachine::WaitLocalSDP(to_peer));
        Ok((to_member_id, event))
    }

    /// Applies an offer to the specified and associated [`Peer`].
    /// Returns [`Event`] to callee that [`Peer`] is created.
    fn handle_make_sdp_offer(
        &mut self,
        from_peer_id: PeerId,
        sdp_offer: String,
    ) -> Result<(MemberId, Event), RoomError> {
        let from_peer = self.peers.take_peer(from_peer_id)?;
        let to_peer_id = from_peer.to_peer();
        let to_peer = self.peers.take_peer(to_peer_id)?;

        let (from_peer, to_peer) = match (from_peer, to_peer) {
            (
                PeerStateMachine::WaitLocalSDP(peer_from),
                PeerStateMachine::New(peer_to),
            ) => Ok((peer_from, peer_to)),
            (from_peer, to_peer) => {
                self.peers.add_peer(from_peer_id, from_peer);
                self.peers.add_peer(to_peer_id, to_peer);
                Err(RoomError::UnmatchedState(from_peer_id, to_peer_id))
            }
        }?;

        let from_peer = from_peer.set_local_sdp(sdp_offer.clone());
        let to_peer = to_peer.set_remote_sdp(sdp_offer.clone());

        let to_member_id = to_peer.member_id();
        let event = Event::PeerCreated {
            peer_id: to_peer_id,
            sdp_offer: Some(sdp_offer),
            tracks: to_peer.tracks(),
        };

        self.peers.add_peer(from_peer_id, PeerStateMachine::WaitRemoteSDP(from_peer));
        self.peers.add_peer(
            to_peer_id,
            PeerStateMachine::WaitLocalHaveRemote(to_peer),
        );
        Ok((to_member_id, event))
    }

    /// Applies an answer to the specified and associated [`Peer`].
    /// Returns [`Event`] to caller that callee has confirmed offer.
    fn handle_make_sdp_answer(
        &mut self,
        from_peer_id: PeerId,
        sdp_answer: String,
    ) -> Result<(MemberId, Event), RoomError> {
        let from_peer = self.peers.take_peer(from_peer_id)?;
        let to_peer_id = from_peer.to_peer();
        let to_peer = self.peers.take_peer(to_peer_id)?;

        let (from_peer, to_peer) = match (from_peer, to_peer) {
            (
                PeerStateMachine::WaitLocalHaveRemote(peer_from),
                PeerStateMachine::WaitRemoteSDP(peer_to),
            ) => Ok((peer_from, peer_to)),
            (from_peer, to_peer) => {
                self.peers.add_peer(from_peer_id, from_peer);
                self.peers.add_peer(to_peer_id, to_peer);
                Err(RoomError::UnmatchedState(from_peer_id, to_peer_id))
            }
        }?;

        let from_peer = from_peer.set_local_sdp(sdp_answer.clone());
        let to_peer = to_peer.set_remote_sdp(&sdp_answer);

        let to_member_id = to_peer.member_id();
        let event = Event::SdpAnswerMade {
            peer_id: to_peer_id,
            sdp_answer,
        };

        self.peers.add_peer(from_peer_id, PeerStateMachine::Stable(from_peer));
        self.peers.add_peer(to_peer_id, PeerStateMachine::Stable(to_peer));
        Ok((to_member_id, event))
    }

    /// Sends Ice Candidate from the specified to the associated [`Peer`].
    fn handle_set_ice_candidate(
        &mut self,
        from_peer_id: PeerId,
        candidate: String,
    ) -> Result<(MemberId, Event), RoomError> {
        let from_peer = self.peers.get_peer(from_peer_id)?;
        let to_peer_id = from_peer.to_peer();
        let to_peer = self.peers.get_peer(to_peer_id)?;

        match (from_peer, to_peer) {
            (
                PeerStateMachine::WaitRemoteSDP(_),
                PeerStateMachine::WaitLocalHaveRemote(_),
            )
            | (
                PeerStateMachine::WaitLocalHaveRemote(_),
                PeerStateMachine::WaitRemoteSDP(_),
            )
            | (PeerStateMachine::Stable(_), PeerStateMachine::Stable(_)) => {
                Ok(())
            }
            _ => Err(RoomError::UnmatchedState(from_peer_id, to_peer_id)),
        }?;

        let to_member_id = to_peer.member_id();
        let event = Event::IceCandidateDiscovered {
            peer_id: to_peer_id,
            candidate,
        };
        Ok((to_member_id, event))
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
        use AuthorizationError::{InvalidCredentials, MemberNotExists};
        if let Some(ref member) = self.members.get(&msg.member_id) {
            if member.credentials.eq(&msg.credentials) {
                return Ok(());
            }
            return Err(InvalidCredentials);
        }
        Err(MemberNotExists)
    }
}

/// Ergonomic type alias for using [`ActorFuture`] for [`Room`].
type ActFuture<I, E> = Box<dyn ActorFuture<Actor = Room, Item = I, Error = E>>;

impl Handler<RpcConnectionEstablished> for Room {
    type Result = ActFuture<(), ()>;

    /// Stores provided [`RpcConnection`] for given [`Member`] in the [`Room`].
    ///
    /// If [`Member`] already has any other [`RpcConnection`],
    /// then it will be closed.
    ///
    /// Initiates media establishment between members.
    fn handle(
        &mut self,
        msg: RpcConnectionEstablished,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        info!("RpcConnectionEstablished for member {}", msg.member_id);

        let mut fut = Either::A(future::ok(()));
        // lookup previous member connection
        if let Some(mut connection) = self.connections.remove(&msg.member_id) {
            debug!("Closing old RpcConnection for member {}", msg.member_id);

            // cancel RpcConnection close task, since connection is reestablished
            if let Some(handler) =
                self.no_reconnect_tasks.remove(&msg.member_id)
            {
                ctx.cancel_future(handler);
            }
            fut = Either::B(connection.close());
        } else {
            self.connections.insert(msg.member_id, msg.connection);
        }

        let peer = self.peers.get_peer_by_member_id(&msg.member_id);
        if let Some(sender) = peer.sender() {
            ctx.notify(ConnectPeers {
                from_peer_id: peer.id(),
                to_peer_id: sender,
            });
        }

        Box::new(wrap_future(fut))
    }
}

/// Signal of close [`Room`].
#[derive(Debug, Message)]
#[rtype(result = "Result<(), ()>")]
#[allow(clippy::module_name_repetitions)]
pub struct CloseRoom {}

impl Handler<CloseRoom> for Room {
    type Result = ActFuture<(), ()>;

    /// Sends to remote [`Member`] the [`Event`] about [`Peer`] removed.
    /// Closes all active [`PrcConnection`].
    fn handle(
        &mut self,
        _msg: CloseRoom,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        use std::mem;
        let connections = mem::replace(&mut self.connections, HashMap::new());
        let fut = connections.into_iter().fold(
            vec![],
            |mut futures, (member_id, mut connection)| {
                info!(
                    "Close connection of member {}, because room is closed",
                    member_id,
                );
                let peer_ids: Vec<_> = self
                    .peers
                    .get_all()
                    .iter()
                    .filter_map(move |peer| match peer {
                        PeerStateMachine::New(_) => None,
                        _ if peer.member_id() == member_id => Some(peer.id()),
                        _ => None,
                    })
                    .collect();
                if peer_ids.is_empty() {
                    futures.push(Either::A(connection.close()));
                } else {
                    futures.push(Either::B(
                        connection
                            .send_event(Event::PeersRemoved { peer_ids })
                            .then(move |_| connection.close()),
                    ));
                }

                futures
            },
        );
        Box::new(wrap_future(join_all(fut).map(|_| ())))
    }
}

/// Signal of start signaling between specified [`Peer`]'s.
#[derive(Debug, Message)]
#[rtype(result = "Result<(), ()>")]
pub struct ConnectPeers {
    pub from_peer_id: PeerId,
    pub to_peer_id: PeerId,
}

impl Handler<ConnectPeers> for Room {
    type Result = ActFuture<(), ()>;

    /// Check state of interconnected [`Peer`]s and sends [`Event`] about
    /// [`Peer`] created to remote [`Member`].
    fn handle(
        &mut self,
        msg: ConnectPeers,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        let addr = ctx.address();
        let fut =
            match self.handle_peer_created(msg.from_peer_id, msg.to_peer_id) {
                Ok((caller, event)) => {
                    Either::A(self.send_event_to_member(caller, event))
                }
                Err(err) => Either::B(future::err(err)),
            }
            .map_err(move |err| {
                error!(
                    "Cannot start signaling between peers {} and {}, because \
                     {}. Room will be stop.",
                    msg.from_peer_id, msg.to_peer_id, err
                );
                addr.do_send(CloseRoom {})
            });
        Box::new(wrap_future(fut))
    }
}

impl Handler<Command> for Room {
    type Result = ActFuture<(), ()>;

    /// Receives [`Command`] from Web client and changes state of interconnected
    /// [`Peer`]s.
    fn handle(
        &mut self,
        command: Command,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        let res = match command {
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
        let addr = ctx.address();
        let fut = match res {
            Ok((caller, event)) => {
                Either::A(self.send_event_to_member(caller, event))
            }
            Err(err) => Either::B(future::err(err)),
        }
        .map_err(move |err| {
            error!(
                "Failed handle command, because {}. Room will be stop.",
                err
            );
            addr.do_send(CloseRoom {})
        });
        Box::new(wrap_future(fut))
    }
}

impl Handler<RpcConnectionClosed> for Room {
    type Result = ();

    /// Removes [`RpcConnection`] of specified [`Member`] from the [`Room`].
    fn handle(&mut self, msg: RpcConnectionClosed, ctx: &mut Self::Context) {
        info!(
            "RpcConnectionClosed for member {}, reason {:?}",
            msg.member_id, msg.reason
        );
        let closed_at = Instant::now();
        let member_id = msg.member_id;
        match msg.reason {
            ClosedReason::Closed => {
                self.connections.remove(&member_id);
                ctx.notify(CloseRoom {})
            }
            ClosedReason::Lost => {
                self.no_reconnect_tasks.insert(
                    msg.member_id,
                    ctx.run_later(self.reconnect_timeout, move |room, ctx| {
                        info!(
                            "Member {} connection lost at {:?}. Room will be \
                             stop.",
                            member_id, closed_at
                        );
                        room.connections.remove(&member_id);
                        ctx.notify(CloseRoom {})
                    }),
                );
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    };

    use actix::{ActorContext, Addr, Arbiter, AsyncContext, System};
    use futures::future::Future;

    use super::*;
    use crate::{
        api::protocol::{
            AudioSettings, Direction, Directional, MediaType, VideoSettings,
        },
        media::peer::create_peers,
    };

    #[derive(Debug, Clone)]
    struct TestConnection {
        pub member_id: MemberId,
        pub room: Addr<Room>,
        pub events: Arc<Mutex<Vec<String>>>,
        pub stopped: Arc<AtomicUsize>,
    }

    impl Actor for TestConnection {
        type Context = Context<Self>;

        fn started(&mut self, ctx: &mut Self::Context) {
            self.room
                .try_send(RpcConnectionEstablished {
                    member_id: self.member_id,
                    connection: Box::new(ctx.address()),
                })
                .unwrap();
        }

        fn stopped(&mut self, _ctx: &mut Self::Context) {
            self.stopped.fetch_add(1, Ordering::Relaxed);
            if self.stopped.load(Ordering::Relaxed) > 1 {
                System::current().stop()
            }
        }
    }

    #[derive(Message)]
    struct Close;

    impl Handler<Close> for TestConnection {
        type Result = ();

        fn handle(&mut self, _: Close, ctx: &mut Self::Context) {
            ctx.stop()
        }
    }

    impl Handler<Event> for TestConnection {
        type Result = ();

        fn handle(&mut self, event: Event, _ctx: &mut Self::Context) {
            let mut events = self.events.lock().unwrap();
            events.push(serde_json::to_string(&event).unwrap());
            match event {
                Event::PeerCreated {
                    peer_id,
                    sdp_offer,
                    tracks: _,
                } => match sdp_offer {
                    Some(_) => self.room.do_send(Command::MakeSdpAnswer {
                        peer_id,
                        sdp_answer: "responder_answer".into(),
                    }),
                    None => self.room.do_send(Command::MakeSdpOffer {
                        peer_id,
                        sdp_offer: "caller_offer".into(),
                    }),
                },
                Event::SdpAnswerMade {
                    peer_id,
                    sdp_answer: _,
                } => self.room.do_send(Command::SetIceCandidate {
                    peer_id,
                    candidate: "ice_candidate".into(),
                }),
                Event::IceCandidateDiscovered {
                    peer_id: _,
                    candidate: _,
                } => {
                    self.room.do_send(RpcConnectionClosed {
                        member_id: self.member_id,
                        reason: ClosedReason::Closed,
                    });
                }
                Event::PeersRemoved { peer_ids: _ } => {}
            }
        }
    }

    impl RpcConnection for Addr<TestConnection> {
        fn close(&mut self) -> Box<dyn Future<Item = (), Error = ()>> {
            let fut = self.send(Close {}).map_err(|_| ());
            Box::new(fut)
        }

        fn send_event(
            &self,
            event: Event,
        ) -> Box<dyn Future<Item = (), Error = ()>> {
            let fut = self.send(event).map_err(|_| ());
            Box::new(fut)
        }
    }

    fn start_room() -> Addr<Room> {
        let members = hashmap! {
            1 => Member{id: 1, credentials: "caller_credentials".to_owned()},
            2 => Member{id: 2, credentials: "responder_credentials".to_owned()},
        };
        Arbiter::start(move |_| {
            Room::new(1, members, create_peers(1, 2), Duration::from_secs(10))
        })
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
            Arbiter::start(move |_| TestConnection {
                events: caller_events_clone,
                member_id: 1,
                room: room_clone,
                stopped: stopped_clone,
            });
            Arbiter::start(move |_| TestConnection {
                events: responder_events_clone,
                member_id: 2,
                room,
                stopped,
            });
        });

        let caller_events = caller_events.lock().unwrap();
        let responder_events = responder_events.lock().unwrap();
        assert_eq!(caller_events.len(), 3);
        assert_eq!(
            caller_events.to_vec(),
            vec![
                serde_json::to_string(&Event::PeerCreated {
                    peer_id: 1,
                    sdp_offer: None,
                    tracks: vec![
                        Directional {
                            id: 1,
                            direction: Direction::Send { receivers: vec![2] },
                            media_type: MediaType::Audio(AudioSettings {}),
                        },
                        Directional {
                            id: 2,
                            direction: Direction::Send { receivers: vec![2] },
                            media_type: MediaType::Video(VideoSettings {}),
                        },
                    ],
                })
                .unwrap(),
                serde_json::to_string(&Event::SdpAnswerMade {
                    peer_id: 1,
                    sdp_answer: "responder_answer".into(),
                })
                .unwrap(),
                serde_json::to_string(&Event::PeersRemoved {
                    peer_ids: vec![1],
                })
                .unwrap(),
            ]
        );
        assert_eq!(responder_events.len(), 2);
        assert_eq!(
            responder_events.to_vec(),
            vec![
                serde_json::to_string(&Event::PeerCreated {
                    peer_id: 2,
                    sdp_offer: Some("caller_offer".into()),
                    tracks: vec![
                        Directional {
                            id: 1,
                            direction: Direction::Recv { sender: 1 },
                            media_type: MediaType::Audio(AudioSettings {}),
                        },
                        Directional {
                            id: 2,
                            direction: Direction::Recv { sender: 1 },
                            media_type: MediaType::Video(VideoSettings {}),
                        },
                    ],
                })
                .unwrap(),
                serde_json::to_string(&Event::IceCandidateDiscovered {
                    peer_id: 2,
                    candidate: "ice_candidate".into(),
                })
                .unwrap(),
            ]
        );
    }

    #[test]
    fn close_responder_connection_without_caller() {
        let stopped = Arc::new(AtomicUsize::new(1));
        let stopped_clone = Arc::clone(&stopped);
        let events = Arc::new(Mutex::new(vec![]));
        let events_clone = Arc::clone(&events);

        System::run(move || {
            let room = start_room();
            Arbiter::start(move |_| TestConnection {
                events,
                member_id: 2,
                room,
                stopped,
            });
        });

        assert_eq!(stopped_clone.load(Ordering::Relaxed), 2);
        let events = events_clone.lock().unwrap();
        assert!(events.is_empty());
    }
}

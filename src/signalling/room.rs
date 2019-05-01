//! Room definitions and implementations.

use std::time::Duration;

use actix::{
    fut::wrap_future, Actor, ActorFuture, AsyncContext, Context, Handler,
    Message,
};
use failure::Fail;
use futures::{
    future::{self, Either},
    Future,
};
use hashbrown::HashMap;

use crate::{
    api::{
        client::rpc_connection::{
            AuthorizationError, Authorize, RpcConnectionClosed,
            RpcConnectionEstablished,
        },
        control::{Member, MemberId},
        protocol::{Command, Event},
    },
    log::prelude::*,
    media::{PeerId, PeerStateMachine},
    signalling::{participants::ParticipantService, peers::PeerRepository},
};

/// ID of [`Room`].
pub type Id = u64;

/// Ergonomic type alias for using [`ActorFuture`] for [`Room`].
type ActFuture<I, E> = Box<dyn ActorFuture<Actor = Room, Item = I, Error = E>>;

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
    #[fail(display = "Generic room error {}", _0)]
    Generic(String),
}

/// Media server room with its [`Member`]s.
#[derive(Debug)]
pub struct Room {
    id: Id,

    /// [`RpcConnection`]s of [`Member`]s in this [`Room`].
    participants: ParticipantService,

    /// [`Peer`]s of [`Member`]s in this [`Room`].
    peers: PeerRepository,
}

impl Room {
    /// Create new instance of [`Room`].
    pub fn new(
        id: Id,
        members: HashMap<MemberId, Member>,
        peers: HashMap<PeerId, PeerStateMachine>,
        reconnect_timeout: Duration,
    ) -> Self {
        Self {
            id,
            peers: PeerRepository::from(peers),
            participants: ParticipantService::new(members, reconnect_timeout),
        }
    }

    /// Applies an offer to the specified and associated [`Peer`].
    /// Returns [`Event`] to callee that [`Peer`] is created.
    fn handle_make_sdp_offer(
        &mut self,
        from_peer_id: PeerId,
        sdp_offer: String,
    ) -> Result<(MemberId, Event), RoomError> {
        let from_peer = self.peers.take_peer(from_peer_id)?;
        let to_peer_id = from_peer.partner_peer_id();
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

        self.peers
            .add_peer(from_peer_id, PeerStateMachine::WaitRemoteSDP(from_peer));
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
        let to_peer_id = from_peer.partner_peer_id();
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

        self.peers
            .add_peer(from_peer_id, PeerStateMachine::Stable(from_peer));
        self.peers
            .add_peer(to_peer_id, PeerStateMachine::Stable(to_peer));
        Ok((to_member_id, event))
    }

    /// Sends Ice Candidate from the specified to the associated [`Peer`].
    fn handle_set_ice_candidate(
        &mut self,
        from_peer_id: PeerId,
        candidate: String,
    ) -> Result<(MemberId, Event), RoomError> {
        let from_peer = self.peers.get_peer(from_peer_id)?;
        let to_peer_id = from_peer.partner_peer_id();
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

    /// Builds [`Event::PeerCreated`].
    /// Both provided peers must be in New state. At least one of provided peers
    /// must have outbound tracks.
    fn build_peer_created(
        &mut self,
        peer1_id: PeerId,
        peer2_id: PeerId,
    ) -> Result<(MemberId, Event), RoomError> {
        let peer1 = self.peers.take_peer(peer1_id)?;
        let peer2 = self.peers.take_peer(peer2_id)?;

        // assert that both Peers are New
        let (peer1, peer2) = match (peer1, peer2) {
            (PeerStateMachine::New(peer1), PeerStateMachine::New(peer2)) => {
                Ok((peer1, peer2))
            }
            (peer1, peer2) => {
                self.peers.add_peer(peer1.id(), peer1);
                self.peers.add_peer(peer2.id(), peer2);
                Err(RoomError::UnmatchedState(peer1_id, peer2_id))
            }
        }?;

        // decide which peer is sender
        let (sender, receiver) = if peer1.is_sender() {
            (peer1, peer2)
        } else if peer2.is_sender() {
            (peer2, peer1)
        } else {
            self.peers
                .add_peer(peer1.id(), PeerStateMachine::New(peer1));
            self.peers
                .add_peer(peer2.id(), PeerStateMachine::New(peer2));
            return Err(RoomError::Generic(format!(
                "Error while trying to connect Peer [id = {}] and Peer [id = \
                 {}] cause neither of peers are senders",
                peer1_id, peer2_id
            )));
        };
        self.peers
            .add_peer(receiver.id(), PeerStateMachine::New(receiver));

        let sender = sender.start();
        let member_id = sender.member_id();
        let peer_created = sender.get_peer_created();
        self.peers
            .add_peer(sender.id(), PeerStateMachine::WaitLocalSDP(sender));
        Ok((member_id, peer_created))
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
        //        let addr = ctx.address();
        //        let fut = self
        //            .participants
        //            .send_event_to_member(member_id, peer_created)
        //            .map_err(|err| {
        //                error!(
        //                    "Cannot start Peer [id = {}], because {}. Stopping
        // room.",                    sender.id(),
        //                    err,
        //                );
        //                addr.do_send(CloseRoom {})
        //            });
        //
        //        Box::new(wrap_future(fut))

        let addr = ctx.address();
        let fut = match self.build_peer_created(msg.0, msg.1) {
            Ok((caller, event)) => {
                Either::A(self.participants.send_event_to_member(caller, event))
            }
            Err(err) => Either::B(future::err(err)),
        }
        .map_err(move |err| {
            error!(
                "Cannot start signaling between peers {} and {}, because {}. \
                 Room will be stop.",
                msg.0, msg.1, err
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
                Either::A(self.participants.send_event_to_member(caller, event))
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

impl Handler<RpcConnectionEstablished> for Room {
    type Result = ActFuture<(), ()>;

    /// Saves new [`RpcConnection`] in [`ParticipantService`], initiates media
    /// establishment between members.
    fn handle(
        &mut self,
        msg: RpcConnectionEstablished,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        info!("RpcConnectionEstablished for member {}", msg.member_id);

        // save new connection
        self.participants.connection_established(
            ctx,
            msg.member_id,
            msg.connection,
        );

        // get connected member Peers
        self.peers
            .get_peers_by_member_id(msg.member_id)
            .into_iter()
            .for_each(|peer| {
                // only New peers should be connected
                if let PeerStateMachine::New(peer) = peer {
                    if self
                        .participants
                        .member_has_connection(peer.partner_member_id())
                    {
                        ctx.notify(ConnectPeers(
                            peer.id(),
                            peer.partner_peer_id(),
                        ));
                    }
                }
            });

        Box::new(wrap_future(future::ok(())))
    }
}

/// Signal of close [`Room`].
#[derive(Debug, Message)]
#[rtype(result = "()")]
#[allow(clippy::module_name_repetitions)]
pub struct CloseRoom {}

impl Handler<CloseRoom> for Room {
    type Result = ();

    /// Sends to remote [`Member`] the [`Event`] about [`Peer`] removed.
    /// Closes all active [`RpcConnection`]s.
    fn handle(
        &mut self,
        _msg: CloseRoom,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        info!("Closing Room [id = {:?}]", self.id);
        let drop_fut = self.participants.drop_connections(ctx);
        ctx.wait(wrap_future(drop_fut));
    }
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
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    };

    use actix::{ActorContext, Addr, Arbiter, AsyncContext, System};
    use futures::future::Future;

    use super::*;
    use crate::{
        api::{
            client::rpc_connection::{ClosedReason, RpcConnection},
            protocol::{
                AudioSettings, Direction, Directional, MediaType, VideoSettings,
            },
        },
        media::create_peers,
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

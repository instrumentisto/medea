//! Room definitions and implementations.

use std::{
    fmt,
    sync::{Arc, Mutex},
};

use actix::{
    fut::wrap_future, Actor, ActorFuture, Addr, AsyncContext, Context, Handler,
    Message,
};
use failure::Fail;
use futures::{
    future::{self, Either},
    Future,
};
use hashbrown::HashMap;

use crate::{
    api::client::{Command, Event, Session},
    api::control::{Id as MemberId, Member},
    log::prelude::*,
    media::{
        peer::{Id as PeerId, Peer, PeerMachine},
        track::{AudioSettings, Track, TrackMediaType, VideoSettings},
    },
};

#[derive(Fail, Debug)]
pub enum RoomError {
    #[fail(display = "Unknown peer {}", _0)]
    UnknownPeer(PeerId),
    #[fail(display = "Unmatched states between peers {} and {}", _0, _1)]
    UnmatchedState(PeerId, PeerId),
}

/// ID of [`Room`].
pub type Id = u64;

/// Media server room with its [`Member`]s.
#[derive(Debug)]
pub struct Room {
    /// ID of this [`Room`].
    pub id: Id,

    /// [`Member`]s which currently are present in this [`Room`].
    pub members: HashMap<MemberId, Member>,

    /// [`Peer`]s of [`Member`]s in this [`Room`].
    pub peers: HashMap<PeerId, PeerMachine>,

    /// [`Session`]s of [`Member`]s with established [`WsConnection`]s.
    pub sessions: HashMap<MemberId, Session>,

    /// Index for generate unique ID for [`Peer`] in this [`Room`].
    peer_index: PeerId,
}

impl Room {
    /// Create new instance of [`Room`].
    pub fn new(id: Id, members: HashMap<MemberId, Member>) -> Self {
        Room {
            id,
            members,
            peers: HashMap::new(),
            sessions: HashMap::new(),
            peer_index: 0,
        }
    }

    /// Store [`Peer`] in [`Room`].
    fn add_peer(&mut self, id: PeerId, peer: PeerMachine) {
        self.peers.insert(id, peer);
    }

    /// Returns borrowed [`Peer`] by its ID.
    fn get_peer(&self, peer_id: &PeerId) -> Result<&PeerMachine, RoomError> {
        self.peers
            .get(peer_id)
            .ok_or(RoomError::UnknownPeer(*peer_id))
    }

    /// Returns ID's all [`Peer`] of specified [`Member`].
    fn member_peers(&mut self, member_id: &MemberId) -> Vec<PeerId> {
        self.peers
            .iter()
            .filter(|(_, peer)| peer.member_id() == *member_id)
            .map(|(&id, _)| id)
            .collect::<Vec<_>>()
    }

    /// Generate next ID of [`Peer`].
    fn next_peer_id(&mut self) -> PeerId {
        self.peer_index += 1;
        self.peer_index
    }

    /// Returns [`Peer`] by its ID.
    fn take_peer(
        &mut self,
        peer_id: &PeerId,
    ) -> Result<PeerMachine, RoomError> {
        self.peers
            .remove(peer_id)
            .ok_or(RoomError::UnknownPeer(*peer_id))
    }

    /// Creates two connected [`Peer`]s and returns [`Event`] to caller
    /// that [`Peer`] is created.
    fn start_pipeline(
        &mut self,
        caller: MemberId,
        callee: MemberId,
    ) -> OnEvent {
        info!("Member {} call member {}", caller, callee);
        let caller_peer_id = self.next_peer_id();
        let callee_peer_id = self.next_peer_id();
        let mut caller_peer = Peer::new(caller_peer_id, caller, callee_peer_id);
        let mut callee_peer = Peer::new(callee_peer_id, callee, caller_peer_id);

        let track_audio =
            Arc::new(Track::new(1, TrackMediaType::Audio(AudioSettings {})));
        let track_video =
            Arc::new(Track::new(2, TrackMediaType::Video(VideoSettings {})));
        caller_peer.add_sender(track_audio.clone());
        caller_peer.add_sender(track_video.clone());
        callee_peer.add_receiver(track_audio);
        callee_peer.add_receiver(track_video);

        let event = OnEvent {
            member_id: caller,
            event: Event::PeerCreated {
                peer_id: caller_peer_id,
                sdp_offer: None,
                tracks: caller_peer.tracks(),
            },
        };
        self.add_peer(
            caller_peer_id,
            PeerMachine::WaitLocalSDP(caller_peer.start()),
        );
        self.add_peer(callee_peer_id, PeerMachine::New(callee_peer));

        event
    }

    /// Applies an offer to the specified and associated [`Peer`].
    /// Returns [`Event`] to callee that [`Peer`] is created.
    fn handle_make_sdp_offer(
        &mut self,
        from_peer_id: PeerId,
        sdp_offer: String,
    ) -> Result<OnEvent, RoomError> {
        let from_peer = self.take_peer(&from_peer_id)?;
        let to_peer_id = from_peer.to_peer();
        let to_peer = self.take_peer(&to_peer_id)?;

        let (from_peer, to_peer) = match (from_peer, to_peer) {
            (
                PeerMachine::WaitLocalSDP(peer_from),
                PeerMachine::New(peer_to),
            ) => Ok((peer_from, peer_to)),
            (from_peer, to_peer) => {
                self.add_peer(from_peer_id, from_peer);
                self.add_peer(to_peer_id, to_peer);
                Err(RoomError::UnmatchedState(from_peer_id, to_peer_id))
            }
        }?;

        let from_peer = from_peer.set_local_sdp(sdp_offer.clone());
        let to_peer = to_peer.set_remote_sdp(sdp_offer.clone());

        let event = OnEvent {
            member_id: to_peer.member_id(),
            event: Event::PeerCreated {
                peer_id: to_peer_id,
                sdp_offer: Some(sdp_offer),
                tracks: to_peer.tracks(),
            },
        };

        self.add_peer(from_peer_id, PeerMachine::WaitRemoteSDP(from_peer));
        self.add_peer(to_peer_id, PeerMachine::WaitLocalHaveRemote(to_peer));
        Ok(event)
    }

    /// Applies an answer to the specified and associated [`Peer`].
    /// Returns [`Event`] to caller that callee has confirmed offer.
    fn handle_make_sdp_answer(
        &mut self,
        from_peer_id: PeerId,
        sdp_answer: String,
    ) -> Result<OnEvent, RoomError> {
        let from_peer = self.take_peer(&from_peer_id)?;
        let to_peer_id = from_peer.to_peer();
        let to_peer = self.take_peer(&to_peer_id)?;

        let (from_peer, to_peer) = match (from_peer, to_peer) {
            (
                PeerMachine::WaitLocalHaveRemote(peer_from),
                PeerMachine::WaitRemoteSDP(peer_to),
            ) => Ok((peer_from, peer_to)),
            (from_peer, to_peer) => {
                self.add_peer(from_peer_id, from_peer);
                self.add_peer(to_peer_id, to_peer);
                Err(RoomError::UnmatchedState(from_peer_id, to_peer_id))
            }
        }?;

        let from_peer = from_peer.set_local_sdp(sdp_answer.clone());
        let to_peer = to_peer.set_remote_sdp(sdp_answer.clone());

        let event = OnEvent {
            member_id: to_peer.member_id(),
            event: Event::SdpAnswerMade {
                peer_id: to_peer_id,
                sdp_answer,
            },
        };

        self.add_peer(from_peer_id, PeerMachine::Stable(from_peer));
        self.add_peer(to_peer_id, PeerMachine::Stable(to_peer));
        Ok(event)
    }

    /// Sends Ice Candidate from the specified to the associated [`Peer`].
    fn handle_set_ice_candidate(
        &mut self,
        from_peer_id: PeerId,
        candidate: String,
    ) -> Result<OnEvent, RoomError> {
        let from_peer = self.get_peer(&from_peer_id)?;
        let to_peer_id = from_peer.to_peer();
        let to_peer = self.get_peer(&to_peer_id)?;

        match (from_peer, to_peer) {
            (
                PeerMachine::WaitRemoteSDP(_),
                PeerMachine::WaitLocalHaveRemote(_),
            )
            | (
                PeerMachine::WaitLocalHaveRemote(_),
                PeerMachine::WaitRemoteSDP(_),
            )
            | (PeerMachine::Stable(_), PeerMachine::Stable(_)) => Ok(()),
            _ => Err(RoomError::UnmatchedState(from_peer_id, to_peer_id)),
        }?;

        Ok(OnEvent {
            member_id: to_peer.member_id(),
            event: Event::IceCandidateDiscovered {
                peer_id: to_peer_id,
                candidate,
            },
        })
    }
}

/// [`Actor`] implementation that provides an ergonomic way
/// to interact with [`Room`].
impl Actor for Room {
    type Context = Context<Self>;
}

/// Established RPC connection with some remote [`Member`].
pub trait RpcConnection: fmt::Debug + Send {
    /// Closes [`RpcConnection`].
    /// No [`RpcConnectionClosed`] signals should be emitted.
    fn close(&self) -> Box<dyn Future<Item = (), Error = ()>>;

    /// Sends [`Event`] to remote [`Member`].
    fn send_event(
        &self,
        event: Event,
    ) -> Box<dyn Future<Item = (), Error = ()>>;
}

/// Signal for authorizing new [`RpcConnection`] before establishing.
#[derive(Debug, Message)]
#[rtype(result = "Result<(), RpcConnectionAuthorizationError>")]
pub struct AuthorizeRpcConnection {
    /// ID of [`Member`] to authorize [`RpcConnection`] for.
    pub member_id: MemberId,
    /// Credentials to authorize [`RpcConnection`] with.
    pub credentials: String, // TODO: &str when futures will allow references
}

/// Error of authorization [`RpcConnection`] in [`Room`].
#[derive(Debug)]
pub enum RpcConnectionAuthorizationError {
    /// Authorizing [`Member`] does not exists in the [`Room`].
    MemberNotExists,
    /// Provided credentials are invalid.
    InvalidCredentials,
}

impl Handler<AuthorizeRpcConnection> for Room {
    type Result = Result<(), RpcConnectionAuthorizationError>;

    /// Responses with `Ok` if `RpcConnection` is authorized, otherwise `Err`s.
    fn handle(
        &mut self,
        msg: AuthorizeRpcConnection,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        use RpcConnectionAuthorizationError::*;
        if let Some(ref member) = self.members.get(&msg.member_id) {
            if member.credentials.eq(&msg.credentials) {
                return Ok(());
            }
            return Err(InvalidCredentials);
        }
        Err(MemberNotExists)
    }
}

/// Signal of new [`RpcConnection`] being established with specified [`Member`].
#[derive(Debug, Message)]
#[rtype(result = "Result<(), ()>")]
pub struct RpcConnectionEstablished {
    /// ID of [`Member`] that establishes [`RpcConnection`].
    pub member_id: MemberId,
    /// Established [`RpcConnection`].
    pub connection: Box<dyn RpcConnection>,
}

/// Ergonomic type alias for using [`ActorFuture`] for [`Room`].
type ActFuture<I, E> = Box<dyn ActorFuture<Actor = Room, Item = I, Error = E>>;

impl Handler<RpcConnectionEstablished> for Room {
    type Result = ActFuture<(), ()>;

    /// Stores provided [`RpcConnection`] for given [`Member`] in the [`Room`].
    ///
    /// If [`Member`] already has any other [`RpcConnection`],
    /// then it will be closed.
    fn handle(
        &mut self,
        msg: RpcConnectionEstablished,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        info!("RpcConnectionEstablished for member {}", msg.member_id);

        let mut fut = Either::A(future::ok(()));
        if let Some(session) = self.sessions.get_mut(&msg.member_id) {
            debug!(
                "Replaced RpcConnection for member {} session",
                msg.member_id
            );
            fut = Either::B(session.set_connection(msg.connection));
        } else {
            let callee = msg.member_id;
            let active_members = self
                .sessions
                .keys()
                .map(|&member_id| member_id)
                .collect::<Vec<_>>();
            self.sessions
                .insert(callee, Session::new(callee, msg.connection));
            active_members.iter().for_each(|&caller| {
                ctx.notify(self.start_pipeline(caller, callee));
            });
        }
        Box::new(wrap_future(fut))
    }
}

impl Handler<Command> for Room {
    type Result = ActFuture<(), RoomError>;

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
        Box::new(wrap_future(future::done(
            res.map(|event| ctx.notify(event)),
        )))
    }
}

/// Signal of existing [`RpcConnection`] of specified [`Member`] being closed.
#[derive(Debug, Message)]
pub struct RpcConnectionClosed {
    /// ID of [`Member`] which [`RpcConnection`] is closed.
    pub member_id: MemberId,
    /// Reason of why [`RpcConnection`] is closed.
    pub reason: RpcConnectionClosedReason,
}

/// Reasons of why [`RpcConnection`] may be closed.
#[derive(Debug)]
pub enum RpcConnectionClosedReason {
    /// [`RpcConnection`] is disconnect by server itself.
    Disconnected,
    /// [`RpcConnection`] has become idle and is disconnected by idle timeout.
    Idle,
}

impl Handler<RpcConnectionClosed> for Room {
    type Result = ();

    /// Removes [`Session`] of specified [`Member`] from the [`Room`].
    fn handle(&mut self, msg: RpcConnectionClosed, ctx: &mut Self::Context) {
        info!(
            "RpcConnectionClosed for member {}, reason {:?}",
            msg.member_id, msg.reason
        );
        self.sessions.remove(&msg.member_id);
        self.member_peers(&msg.member_id)
            .iter()
            .for_each(|peer_id| {
                self.peers
                    .remove(peer_id)
                    .and_then(|peer| self.peers.remove(&peer.to_peer()))
                    .map(|to_peer| {
                        ctx.notify(OnEvent {
                            member_id: to_peer.member_id(),
                            event: Event::PeerFinished {
                                peer_id: to_peer.id(),
                            },
                        })
                    });
            })
    }
}

#[derive(Debug, Message)]
pub struct OnEvent {
    member_id: MemberId,
    event: Event,
}

impl Handler<OnEvent> for Room {
    type Result = ();

    /// Sends [`Event`] to specified [`Member`] of [`Room`].
    fn handle(&mut self, msg: OnEvent, ctx: &mut Self::Context) {
        if let Some(session) = self.sessions.get(&msg.member_id) {
            ctx.wait(wrap_future(session.send_event(msg.event)));
        }
    }
}

/// Repository that stores [`Room`]s.
#[derive(Clone, Default)]
pub struct RoomsRepository {
    rooms: Arc<Mutex<HashMap<Id, Addr<Room>>>>,
}

impl RoomsRepository {
    /// Creates new [`Room`]s repository with passed-in [`Room`]s.
    pub fn new(rooms: HashMap<Id, Addr<Room>>) -> Self {
        Self {
            rooms: Arc::new(Mutex::new(rooms)),
        }
    }

    /// Returns [`Room`] by its ID.
    pub fn get(&self, id: Id) -> Option<Addr<Room>> {
        let rooms = self.rooms.lock().unwrap();
        rooms.get(&id).cloned()
    }
}

#[cfg(test)]
mod test {
    use actix::{ActorContext, Arbiter, AsyncContext, System};
    use futures::future::Future;

    use super::*;

    #[derive(Debug, Clone)]
    struct TestConnection {
        pub member_id: MemberId,
        pub room: Addr<Room>,
        pub events: Arc<Mutex<Vec<String>>>,
    }

    impl Actor for TestConnection {
        type Context = Context<Self>;

        fn started(&mut self, ctx: &mut Self::Context) {
            let caller_message = RpcConnectionEstablished {
                member_id: self.member_id,
                connection: Box::new(ctx.address()),
            };
            self.room.do_send(caller_message);
        }
    }

    impl Handler<Event> for TestConnection {
        type Result = ();

        fn handle(&mut self, event: Event, ctx: &mut Self::Context) {
            let mut events = self.events.lock().unwrap();
            events.push(serde_json::to_string(&event).unwrap());
            match event {
                Event::PeerCreated {
                    peer_id,
                    sdp_offer,
                    tracks,
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
                        reason: RpcConnectionClosedReason::Disconnected,
                    });
                    ctx.stop();
                }
                Event::PeerFinished { peer_id: _ } => {
                    System::current().stop();
                }
            }
        }
    }

    impl RpcConnection for Addr<TestConnection> {
        fn close(&self) -> Box<dyn Future<Item = (), Error = ()>> {
            Box::new(future::ok(()))
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
        Arbiter::start(move |_| Room::new(1, members))
    }

    #[test]
    fn start_signaling() {
        let caller_events = Arc::new(Mutex::new(vec![]));
        let caller_events_clone = Arc::clone(&caller_events);
        let responder_events = Arc::new(Mutex::new(vec![]));
        let responder_events_clone = Arc::clone(&responder_events);

        System::run(move || {
            let room = start_room();
            let room_clone = room.clone();
            Arbiter::start(move |_| TestConnection {
                member_id: 1,
                room: room_clone,
                events: caller_events_clone,
            });
            let room_clone = room.clone();
            Arbiter::start(move |_| TestConnection {
                member_id: 2,
                room: room_clone,
                events: responder_events_clone,
            });
        });

        let mut caller_events = caller_events.lock().unwrap();
        let responder_events = responder_events.lock().unwrap();
        assert_eq!(caller_events.len(), 3);
        assert_eq!(
            caller_events.to_vec(),
            vec![
                "{\"PeerCreated\":{\"peer_id\":1,\"sdp_offer\":null,\
                 \"tracks\":[{\"id\":1,\"media_type\":{\"Audio\":{}},\
                 \"direction\":{\"Send\":{\"receivers\":[2]}}},{\"id\":2,\
                 \"media_type\":{\"Video\":{}},\"direction\":{\"Send\":\
                 {\"receivers\":[2]}}}]}}",
                "{\"SdpAnswerMade\":{\"peer_id\":1,\"sdp_answer\":\
                 \"responder_answer\"}}",
                "{\"PeerFinished\":{\"peer_id\":1}}"
            ]
        );
        assert_eq!(responder_events.len(), 2);
        assert_eq!(
            responder_events.to_vec(),
            vec![
                "{\"PeerCreated\":{\"peer_id\":2,\"sdp_offer\":\
                 \"caller_offer\",\"tracks\":[{\"id\":1,\"media_type\":\
                 {\"Audio\":{}},\"direction\":{\"Recv\":{\"sender\":1}}},\
                 {\"id\":2,\"media_type\":{\"Video\":{}},\"direction\":\
                 {\"Recv\":{\"sender\":1}}}]}}",
                "{\"IceCandidateDiscovered\":{\"peer_id\":2,\"candidate\":\
                 \"ice_candidate\"}}",
            ]
        );
    }
}

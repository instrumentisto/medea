//! Room definitions and implementations.

use std::{
    fmt,
    sync::{Arc, Mutex},
};

use actix::{
    fut::wrap_future, Actor, ActorFuture, Addr, Context, Handler, Message,
};
use failure::Fail;
use futures::{
    future::{self, Either},
    Future,
};
use hashbrown::HashMap;

use crate::{
    api::client::{Command, Event},
    api::control::{Id as MemberId, Member},
    log::prelude::*,
    media::{
        AudioSettings, Peer, PeerId, PeerMachine, Track, TrackMediaType,
        VideoSettings,
    },
};

/// ID of [`Room`].
pub type Id = u64;

/// Media server room with its [`Member`]s.
#[derive(Debug)]
pub struct Room {
    /// ID of this [`Room`].
    pub id: Id,
    /// [`Member`]s which currently are present in this [`Room`].
    pub members: HashMap<MemberId, Member>,
    /// Established [`WsSession`]s of [`Member`]s in this [`Room`].
    pub connections: HashMap<MemberId, Box<dyn RpcConnection>>,
    // TODO: Replace Box<dyn RpcConnection>> with enum,
    //       as the set of all possible RpcConnection types is not closed.
    /// Peers of [`Member`]'s this room.
    member_peers: HashMap<MemberId, PeerId>,

    /// Relations peer to peer.
    peer_to_peer: HashMap<PeerId, PeerId>,

    /// [`Peer`]s of [`Member`]'s this room.
    peers: HashMap<PeerId, PeerMachine>,

    peer_index: u64,
}

impl Room {
    /// Create new instance of [`Room`].
    pub fn new(id: Id, members: HashMap<MemberId, Member>) -> Self {
        Room {
            id,
            members,
            connections: HashMap::new(),
            member_peers: HashMap::new(),
            peers: HashMap::new(),
            peer_to_peer: HashMap::new(),
            peer_index: 0,
        }
    }

    /// Generate next ID of [`Peer`].
    fn next_peer_id(&mut self) -> PeerId {
        let id = self.peer_index;
        self.peer_index += 1;
        id
    }

    fn remove_peer_by_id(
        &mut self,
        peer_id: &PeerId,
    ) -> Result<PeerMachine, RoomError> {
        self.peers
            .remove(peer_id)
            .ok_or(RoomError::UnknownPeer(*peer_id))
    }

    fn get_peer_by_id(
        &self,
        peer_id: &PeerId,
    ) -> Result<&PeerMachine, RoomError> {
        self.peers
            .get(peer_id)
            .ok_or(RoomError::UnknownPeer(*peer_id))
    }

    /// Begins the negotiation process between peers.
    ///
    /// Creates audio and video tracks and stores links to them in
    /// interconnected peers.
    fn start_pipeline(
        &mut self,
        caller: MemberId,
        responder: MemberId,
    ) -> Result<(), RoomError> {
        let peer_caller_id = self
            .member_peers
            .get(&caller)
            .map(|id| *id)
            .ok_or(RoomError::MemberWithoutPeer(caller))?;
        let peer_responder_id = self
            .member_peers
            .get(&responder)
            .map(|id| *id)
            .ok_or(RoomError::MemberWithoutPeer(responder))?;
        self.peer_to_peer.insert(peer_caller_id, peer_responder_id);
        self.peer_to_peer.insert(peer_responder_id, peer_caller_id);

        let track_audio =
            Arc::new(Track::new(1, TrackMediaType::Audio(AudioSettings {})));
        let track_video =
            Arc::new(Track::new(2, TrackMediaType::Video(VideoSettings {})));

        let peer_caller = self
            .peers
            .remove(&peer_caller_id)
            .ok_or(RoomError::UnknownPeer(peer_caller_id))?;

        let new_peer_caller = match peer_caller {
            PeerMachine::New(mut peer) => {
                peer.add_sender(track_audio.clone());
                peer.add_sender(track_video.clone());
                Ok(PeerMachine::WaitLocalSDP(peer.start(
                    peer_responder_id,
                    |member_id, event| {
                        let connection =
                            self.connections.get(&member_id).unwrap();
                        connection.send_event(event);
                    },
                )))
            }
            _ => Err(RoomError::UnmatchedState(peer_caller_id)),
        }?;
        self.peers.insert(peer_caller_id, new_peer_caller);

        let peer_responder = self
            .peers
            .remove(&peer_responder_id)
            .ok_or(RoomError::UnknownPeer(peer_responder_id))?;

        let new_peer_responder = match peer_responder {
            PeerMachine::New(mut peer) => {
                peer.add_receiver(track_audio);
                peer.add_receiver(track_video);
                Ok(PeerMachine::New(peer))
            }
            _ => Err(RoomError::UnmatchedState(peer_responder_id)),
        }?;
        self.peers.insert(peer_responder_id, new_peer_responder);
        Ok(())
    }

    fn handle_make_sdp_offer(
        &mut self,
        from_peer_id: PeerId,
        sdp_offer: String,
    ) -> Result<(), RoomError> {
        let from_peer = self.remove_peer_by_id(&from_peer_id)?;
        let from_peer = match from_peer {
            PeerMachine::WaitLocalSDP(peer) => Ok(PeerMachine::WaitRemoteSDP(
                peer.set_local_sdp(sdp_offer.clone()),
            )),
            _ => {
                error!("Unmatched state caller peer");
                Err(RoomError::UnmatchedState(from_peer_id))
            }
        }?;

        self.peers.insert(from_peer_id, from_peer);

        let responder_peer_id = self
            .peer_to_peer
            .get(&from_peer_id)
            .map(|&id| id)
            .ok_or(RoomError::NoOpponentPeer(from_peer_id))?;
        let peer_responder = self
            .peers
            .remove(&responder_peer_id)
            .ok_or(RoomError::UnknownPeer(responder_peer_id))?;
        let new_peer_responder = match peer_responder {
            PeerMachine::New(peer) => {
                Ok(PeerMachine::WaitLocalHaveRemote(peer.set_remote_sdp(
                    from_peer_id,
                    sdp_offer,
                    |member_id, event| {
                        let connection =
                            self.connections.get(&member_id).unwrap();
                        connection.send_event(event);
                    },
                )))
            }
            _ => {
                error!("Unmatched state responder peer");
                Err(RoomError::UnmatchedState(responder_peer_id))
            }
        }?;
        self.peers.insert(responder_peer_id, new_peer_responder);
        Ok(())
    }

    fn handle_make_sdp_answer(
        &mut self,
        from_peer_id: PeerId,
        sdp_answer: String,
    ) -> Result<(), RoomError> {
        let from_peer = self.remove_peer_by_id(&from_peer_id)?;
        let from_peer = match from_peer {
            PeerMachine::WaitLocalHaveRemote(peer) => {
                Ok(PeerMachine::Stable(peer.set_local_sdp(sdp_answer.clone())))
            }
            _ => {
                error!("Unmatched state caller peer");
                Err(RoomError::UnmatchedState(from_peer_id))
            }
        }?;
        self.peers.insert(from_peer_id, from_peer);

        let caller_peer_id = self
            .peer_to_peer
            .get(&from_peer_id)
            .map(|&id| id)
            .ok_or(RoomError::NoOpponentPeer(from_peer_id))?;
        let peer_caller = self
            .peers
            .remove(&caller_peer_id)
            .ok_or(RoomError::UnknownPeer(caller_peer_id))?;
        let new_peer_caller = match peer_caller {
            PeerMachine::WaitRemoteSDP(peer) => {
                Ok(PeerMachine::Stable(peer.set_remote_sdp(
                    sdp_answer,
                    |peer_id, member_id, sdp_answer| {
                        let connection =
                            self.connections.get(&member_id).unwrap();
                        let event = Event::SdpAnswerMade {
                            peer_id,
                            sdp_answer,
                        };
                        connection.send_event(event);
                    },
                )))
            }
            _ => Err(RoomError::UnmatchedState(caller_peer_id)),
        }?;
        self.peers.insert(caller_peer_id, new_peer_caller);

        Ok(())
    }

    fn handle_set_ice_candidate(
        &mut self,
        from_peer_id: PeerId,
        candidate: String,
    ) -> Result<(), RoomError> {
        fn send_candidate_to_remote(
            room: &Room,
            from_peer_id: PeerId,
            candidate: String,
        ) -> Result<(), RoomError> {
            let remote_peer_id = room
                .peer_to_peer
                .get(&from_peer_id)
                .ok_or(RoomError::NoOpponentPeer(from_peer_id))?;

            let remote = room
                .peers
                .get(remote_peer_id)
                .ok_or(RoomError::UnknownPeer(*remote_peer_id))?;

            let remote_member_id = remote.get_member_id();
            let remote = room
                .connections
                .get(&remote_member_id)
                .ok_or(RoomError::InvalidConnection(remote_member_id))?;

            remote.send_event(Event::IceCandidateDiscovered {
                peer_id: *remote_peer_id,
                candidate,
            });

            Ok(())
        };

        let from_peer = self.remove_peer_by_id(&from_peer_id)?;
        match from_peer {
            PeerMachine::WaitLocalSDP(_) => {
                send_candidate_to_remote(self, from_peer_id, candidate)
            }
            PeerMachine::WaitLocalHaveRemote(_) => {
                send_candidate_to_remote(self, from_peer_id, candidate)
            }
            PeerMachine::WaitRemoteSDP(_) => {
                send_candidate_to_remote(self, from_peer_id, candidate)
            }
            PeerMachine::Stable(_) => {
                send_candidate_to_remote(self, from_peer_id, candidate)
            }
            _ => Err(RoomError::UnmatchedState(from_peer_id)),
        }
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
    fn close(&mut self) -> Box<dyn Future<Item = (), Error = ()>>;

    fn send_event(&self, event: Event);
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

#[derive(Fail, Debug)]
pub enum RoomError {
    #[fail(display = "Member without peer {}", _0)]
    MemberWithoutPeer(MemberId),
    #[fail(display = "Invalid connection of member {}", _0)]
    InvalidConnection(MemberId),
    #[fail(display = "Unknown peer {}", _0)]
    UnknownPeer(PeerId),
    #[fail(display = "Peer dont have opponent {}", _0)]
    NoOpponentPeer(PeerId),
    #[fail(display = "Unmatched state of peer {}", _0)]
    UnmatchedState(PeerId),
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
type ActFuture<I, E> =
    Box<dyn ActorFuture<Actor = Room, Item = I, Error = E> + 'static>;

impl Handler<RpcConnectionEstablished> for Room {
    type Result = ActFuture<(), ()>;

    /// Stores provided [`RpcConnection`] for given [`Member`] in the [`Room`].
    ///
    /// If [`Member`] already has any other [`RpcConnection`],
    /// then it will be closed.
    fn handle(
        &mut self,
        msg: RpcConnectionEstablished,
        _: &mut Self::Context,
    ) -> Self::Result {
        info!("RpcConnectionEstablished for member {}", msg.member_id);

        let mut fut = Either::A(future::ok(()));
        let mut reconnected = false;

        if let Some(mut old_conn) = self.connections.remove(&msg.member_id) {
            debug!("Closing old RpcConnection for member {}", msg.member_id);
            fut = Either::B(old_conn.close());
            reconnected = true;
        }

        self.connections.insert(msg.member_id, msg.connection);

        if !reconnected {
            let peer_id = self.next_peer_id();
            let peer = PeerMachine::New(Peer::new(peer_id, msg.member_id));
            self.peers.insert(peer_id, peer);
            self.member_peers.insert(msg.member_id, peer_id);

            info!("count connections: {}", self.connections.len());
            if self.connections.len() > 1 {
                info!("Pipeline started.");
                self.start_pipeline(1, 2).map_err(|err| {
                    error!("Failed start pipeline, because: {}", err)
                });
            }
        }

        Box::new(wrap_future(fut))
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

    /// Removes [`RpcConnection`] of specified [`Member`] from the [`Room`].
    fn handle(&mut self, msg: RpcConnectionClosed, _: &mut Self::Context) {
        info!(
            "RpcConnectionClosed for member {}, reason {:?}",
            msg.member_id, msg.reason
        );
        self.connections.remove(&msg.member_id);
    }
}

impl Handler<Command> for Room {
    type Result = ActFuture<(), RoomError>;

    /// Receives [`Command`] from Web client and changes state of interconnected
    /// [`Peer`]s.
    fn handle(
        &mut self,
        command: Command,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        debug!("receive command: {:?}", command);
        let fut = match command {
            Command::MakeSdpOffer { peer_id, sdp_offer } => {
                future::done(self.handle_make_sdp_offer(peer_id, sdp_offer))
            }
            Command::MakeSdpAnswer {
                peer_id,
                sdp_answer,
            } => future::done(self.handle_make_sdp_answer(peer_id, sdp_answer)),
            Command::SetIceCandidate { peer_id, candidate } => {
                future::done(self.handle_set_ice_candidate(peer_id, candidate))
            }
        };
        Box::new(wrap_future(fut))
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
    use std::time::{Duration, Instant};

    use actix::{Arbiter, AsyncContext, System};
    use futures::future::{result, Future};
    use tokio::timer::Delay;

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
                    peer_id: _,
                    sdp_answer: _,
                } => {
                    System::current().stop();
                }
                Event::IceCandidateDiscovered {
                    peer_id: _,
                    candidate: _,
                } => {}
            }
        }
    }

    impl RpcConnection for Addr<TestConnection> {
        fn close(&mut self) -> Box<dyn Future<Item = (), Error = ()>> {
            Box::new(future::ok(()))
        }

        fn send_event(&self, event: Event) {
            self.do_send(event);
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
        assert_eq!(caller_events.len(), 2);
        assert_eq!(
            caller_events.to_vec(),
            vec![
                "{\"PeerCreated\":{\"peer_id\":0,\"sdp_offer\":null,\
                 \"tracks\":[{\"id\":1,\"media_type\":{\"Audio\":{}},\
                 \"direction\":{\"Send\":{\"receivers\":[1]}}},{\"id\":2,\
                 \"media_type\":{\"Video\":{}},\"direction\":{\"Send\":\
                 {\"receivers\":[1]}}}]}}",
                "{\"SdpAnswerMade\":{\"peer_id\":0,\"sdp_answer\":\
                 \"responder_answer\"}}",
            ]
        );
        assert_eq!(responder_events.len(), 1);
    }
}

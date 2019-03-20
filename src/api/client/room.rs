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
        peer::{Id as PeerId, Peer, PeerMachine, Transceiver},
        track::{
            AudioSettings, Id as TrackId, Track, TrackMediaType, VideoSettings,
        },
    },
};

lazy_static! {
    static ref PEER_INDEX: Mutex<u64> = Mutex::new(0);
}

/// Generate next ID of [`Peer`].
fn next_peer_id() -> PeerId {
    let mut index = PEER_INDEX.lock().unwrap();
    *index += 1;
    *index
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

/// ID of [`Room`].
pub type Id = u64;

/// Media server room with its [`Member`]s.
#[derive(Debug)]
pub struct Room {
    /// ID of this [`Room`].
    pub id: Id,
    /// [`Member`]s which currently are present in this [`Room`].
    pub members: HashMap<MemberId, Member>,
    /// [`Session`]s of [`Member`]s in this [`Room`].
    pub sessions: HashMap<MemberId, Session>,
    /* TODO: Replace Box<dyn RpcConnection>> with enum,
     *       as the set of all possible RpcConnection types is not closed. */
}

impl Room {
    /// Create new instance of [`Room`].
    pub fn new(id: Id, members: HashMap<MemberId, Member>) -> Self {
        Room {
            id,
            members,
            sessions: HashMap::new(),
        }
    }

    pub fn remove_session(&mut self, member_id: MemberId) {
        if let Some(session) = self.sessions.remove(&member_id) {
            for peer in session.peers.values() {
                if let Some(transceiver_session) =
                    self.sessions.get_mut(&peer.transceiver().member_id)
                {
                    info!(
                        "Remove peer {:?} of member {:?}",
                        peer.transceiver().peer_id,
                        peer.transceiver().member_id
                    );
                    transceiver_session.remove_peer(peer.transceiver().peer_id);
                }
            }
        }
    }

    fn handle_make_sdp_offer(
        &mut self,
        from_member_id: MemberId,
        from_peer_id: PeerId,
        sdp_offer: String,
    ) -> Result<MemberEvent, RoomError> {
        let from_session = self
            .sessions
            .get_mut(&from_member_id)
            .ok_or(RoomError::UnknownPeer(from_member_id))?;
        let from_peer = from_session
            .remove_peer(from_peer_id)
            .ok_or(RoomError::UnknownPeer(from_member_id))?;
        let transceiver = match from_peer {
            PeerMachine::WaitLocalSDP(peer) => {
                let from_peer = peer.set_local_sdp(sdp_offer.clone());
                let trans = from_peer.transceiver();
                from_session.add_peer(PeerMachine::WaitRemoteSDP(from_peer));
                Ok(trans)
            }
            _ => {
                let peer_id = from_peer.id();
                error!("Unmatched state caller peer {}", peer_id);
                from_session.add_peer(from_peer);
                Err(RoomError::UnmatchedState(peer_id))
            }
        }?;

        let to_session = self
            .sessions
            .get_mut(&transceiver.member_id)
            .ok_or(RoomError::UnknownPeer(transceiver.peer_id))?;
        let to_peer = to_session
            .remove_peer(transceiver.peer_id)
            .ok_or(RoomError::UnknownPeer(transceiver.peer_id))?;

        let event = match to_peer {
            PeerMachine::New(peer) => {
                let to_peer = peer.set_remote_sdp(sdp_offer.clone());
                let event = MemberEvent {
                    member_id: transceiver.member_id,
                    event: Event::PeerCreated {
                        peer_id: to_peer.id(),
                        sdp_offer: Some(sdp_offer),
                        tracks: to_peer.tracks(),
                    },
                };
                to_session.add_peer(PeerMachine::WaitLocalHaveRemote(to_peer));
                Ok(event)
            }
            _ => {
                let peer_id = to_peer.id();
                error!("Unmatched state responder peer {}", peer_id);
                to_session.add_peer(to_peer);
                Err(RoomError::UnmatchedState(peer_id))
            }
        }?;

        Ok(event)
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
            let member_id = msg.member_id;
            let mut session = Session::new(msg.member_id, msg.connection);

            info!("Members in room: {:?}", self.sessions.len());
            let events = self
                .sessions
                .iter_mut()
                .filter(|&(&m_id, _)| m_id != member_id)
                .fold(vec![], |mut events, (_, callee)| {
                    events.push(start_pipeline(&mut session, callee));
                    events
                });
            self.sessions.insert(member_id, session);
            events.into_iter().for_each(|e| {
                ctx.notify(MemberEvent {
                    member_id,
                    event: e,
                })
            })
        }

        Box::new(wrap_future(fut))
    }
}

fn start_pipeline(caller: &mut Session, callee: &mut Session) -> Event {
    info!(
        "Member {} call member {}",
        caller.member_id, callee.member_id
    );
    let caller_peer_id = next_peer_id();
    let callee_peer_id = next_peer_id();
    let mut caller_peer = Peer::new(
        caller_peer_id,
        caller.member_id,
        Transceiver {
            member_id: callee.member_id,
            peer_id: callee_peer_id,
        },
    );
    let mut callee_peer = Peer::new(
        callee_peer_id,
        callee.member_id,
        Transceiver {
            member_id: caller.member_id,
            peer_id: caller_peer_id,
        },
    );

    let track_audio =
        Arc::new(Track::new(1, TrackMediaType::Audio(AudioSettings {})));
    let track_video =
        Arc::new(Track::new(2, TrackMediaType::Video(VideoSettings {})));
    caller_peer.add_sender(track_audio.clone());
    caller_peer.add_sender(track_video.clone());
    callee_peer.add_receiver(track_audio);
    callee_peer.add_receiver(track_video);

    let event = Event::PeerCreated {
        peer_id: caller_peer.id(),
        sdp_offer: None,
        tracks: caller_peer.tracks(),
    };
    let caller_peer = PeerMachine::WaitLocalSDP(caller_peer.start());
    caller.add_peer(caller_peer);

    let callee_peer = PeerMachine::New(callee_peer);
    callee.add_peer(callee_peer);

    event
}

#[derive(Debug, Message)]
#[rtype(result = "Result<(), RoomError>")]
pub struct MemberCommand {
    pub member_id: MemberId,
    pub command: Command,
}

impl Handler<MemberCommand> for Room {
    type Result = ActFuture<(), RoomError>;

    /// Receives [`Command`] from Web client and changes state of interconnected
    /// [`Peer`]s.
    fn handle(
        &mut self,
        msg: MemberCommand,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        debug!("receive command: {:?}", msg);
        let fut = match msg.command {
            Command::MakeSdpOffer { peer_id, sdp_offer } => future::done(
                self.handle_make_sdp_offer(msg.member_id, peer_id, sdp_offer)
                    .map(|event| ctx.notify(event)),
            ),
            _ => future::ok(()), /* Command::MakeSdpAnswer {
                                  * peer_id,
                                  * sdp_answer,
                                  * } => future::done(self.
                                  * handle_make_sdp_answer(peer_id,
                                  * sdp_answer)),
                                  * Command::SetIceCandidate { peer_id,
                                  * candidate } => {
                                  * future::done(self.
                                  * handle_set_ice_candidate(peer_id,
                                  * candidate))
                                  * } */
        };
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

    /// Removes [`Session`] of specified [`Member`] from the [`Room`].
    fn handle(&mut self, msg: RpcConnectionClosed, _: &mut Self::Context) {
        info!(
            "RpcConnectionClosed for member {}, reason {:?}",
            msg.member_id, msg.reason
        );
        self.remove_session(msg.member_id);
    }
}

#[derive(Debug, Message)]
struct MemberEvent {
    member_id: MemberId,
    event: Event,
}

impl Handler<MemberEvent> for Room {
    type Result = ();

    /// Send [`Event`] to specified [`Member`] from the [`Room`].
    fn handle(&mut self, msg: MemberEvent, ctx: &mut Self::Context) {
        let member_id = msg.member_id;
        info!("Send event {:?} for member {}", msg.event, msg.member_id);
        if let Some(session) = self.sessions.get(&member_id) {
            ctx.wait(wrap_future(session.send_event(msg.event)))
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

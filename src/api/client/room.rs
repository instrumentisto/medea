//! Room definitions and implementations.

use std::{
    fmt,
    sync::{Arc, Mutex},
};

use actix::{
    fut::wrap_future, Actor, ActorFuture, Addr, AsyncContext, Context, Handler,
    Message,
};
use futures::{
    future::{self, join_all, Either},
    Future,
};
use hashbrown::HashMap;

use crate::{
    api::client::{Event, Session},
    api::control::{Id as MemberId, Member},
    log::prelude::*,
    media::{
        peer::{Id as PeerId, Peer, PeerMachine},
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

    pub fn session_by_peer(
        &mut self,
        peer_id: PeerId,
    ) -> Option<(&u64, &mut Session)> {
        self.sessions
            .iter_mut()
            .find(|(_, s)| s.peers.contains_key(&peer_id))
    }

    pub fn remove_session(&mut self, member_id: MemberId) {
        if let Some(session) = self.sessions.remove(&member_id) {
            for peer in session.peers.values() {
                let opponent_peer_id = peer.opponent_id();
                if let Some((_, opp_session)) =
                    self.session_by_peer(opponent_peer_id)
                {
                    info!(
                        "Remove peer {:?} of member {:?}",
                        opponent_peer_id, opp_session.member_id
                    );
                    opp_session.remove_peer(opponent_peer_id);
                }
            }
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
    let mut caller_peer =
        Peer::new(caller_peer_id, caller.member_id, callee_peer_id);
    let mut callee_peer =
        Peer::new(callee_peer_id, callee.member_id, caller_peer_id);

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

    info!("Send event: {:?}", event);
    event
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

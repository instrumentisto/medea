//! Room definitions and implementations.

use std::{
    fmt,
    sync::{Arc, Mutex},
};

use actix::{
    fut::wrap_future, Actor, ActorFuture, Addr, Context, Handler, Message,
};
use futures::{
    future::{self, Either},
    Future,
};
use hashbrown::HashMap;

use crate::{
    api::control::{Id as MemberId, Member},
    log::prelude::*,
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
    /* TODO: Replace Box<dyn RpcConnection>> with enum,
     *       as the set of all possible RpcConnection types is not closed. */
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
        _: &mut Self::Context,
    ) -> Self::Result {
        info!("RpcConnectionEstablished for member {}", msg.member_id);

        let mut fut = Either::A(future::ok(()));

        if let Some(mut old_conn) = self.connections.remove(&msg.member_id) {
            debug!("Closing old RpcConnection for member {}", msg.member_id);
            fut = Either::B(old_conn.close());
        }

        self.connections.insert(msg.member_id, msg.connection);

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

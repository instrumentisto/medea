//! Room definitions and implementations.
use std::sync::{Arc, Mutex};

use actix::prelude::*;
use hashbrown::HashMap;

use crate::{
    api::control::{Id as MemberID, Member},
    log::prelude::*,
};
use std::fmt::Debug;

/// ID of [`Room`].
pub type Id = u64;

/// Media server room with its members.
#[derive(Debug)]
pub struct Room {
    /// ID of [`Room`].
    pub id: Id,

    /// [`Member`]'s this room.
    pub members: HashMap<MemberID, Member>,

    /// [`WsSession`]s of [`Member`]'s this room.
    pub connections: HashMap<MemberID, Box<dyn RpcConnection>>,
}

/// [`Actor`] implementation that provides an ergonomic way for members
/// to interact in [`Room`].
impl Actor for Room {
    type Context = Context<Self>;
}

/// [`RpcConnection`] with remote [`Room`] [`Member`].
pub trait RpcConnection: Debug + Send {
    /// Close connection. No [`RpcConnectionClosed`] should be emitted.
    fn close(&self);
}

/// Signals that new  [`RpcConnection`] was established with specified [`Member`].
#[derive(Message, Debug)]
pub struct RpcConnectionEstablished {
    pub member_id: MemberID,
    pub connection: Box<dyn RpcConnection>,
}

/// Requests [`Member`] by credentials.
#[derive(Message, Debug)]
#[rtype(result = "Option<Member>")]
pub struct GetMember {
    pub credentials: String,
}

/// Signals that [`RpcConnection`] with specified member was closed.
#[derive(Message, Debug)]
pub struct RpcConnectionClosed {
    pub member_id: MemberID,
    pub reason: RpcConnectionClosedReason,
}

/// [`RpcConnection`] close reasons.
#[derive(Debug)]
pub enum RpcConnectionClosedReason {
    /// [`RpcConnection`] initiated disconnect from server.
    Disconnect,
    /// [`RpcConnection`] was considered idle and disconnected.
    Idle,
}

impl Handler<GetMember> for Room {
    type Result = Option<Member>;

    /// Returns [`Member`] by its credentials, if any.
    fn handle(
        &mut self,
        msg: GetMember,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        self.members
            .values()
            .find(|m| m.credentials.eq(&msg.credentials))
            .map(|m| m.clone())
    }
}

impl Handler<RpcConnectionEstablished> for Room {
    type Result = ();

    /// Stores provided [`RPCConnection`] with specified [`Member`] into [`Room`].
    ///
    /// Current [`RPCConnection`] with specified ['Member'] will be closed, if any.
    fn handle(
        &mut self,
        msg: RpcConnectionEstablished,
        _ctx: &mut Self::Context,
    ) {
        info!("RpcConnectionEstablished with member {}", &msg.member_id);
        if let Some(old_connection) = self.connections.remove(&msg.member_id) {
            debug!("New RpcConnection with member {}", msg.member_id);
            old_connection.close();
        }
        self.connections.insert(msg.member_id, msg.connection);
    }
}

impl Handler<RpcConnectionClosed> for Room {
    type Result = ();

    /// Remove connection of [`Member`] from [`Room`].
    fn handle(&mut self, msg: RpcConnectionClosed, _ctx: &mut Self::Context) {
        info!(
            "RpcConnectionClosed with member {}, reason {:?}",
            &msg.member_id, msg.reason
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
        RoomsRepository {
            rooms: Arc::new(Mutex::new(rooms)),
        }
    }

    /// Returns [`Room`] by its ID.
    pub fn get(&self, id: Id) -> Option<Addr<Room>> {
        let rooms = self.rooms.lock().unwrap();
        rooms.get(&id).map(|r| r.clone())
    }
}

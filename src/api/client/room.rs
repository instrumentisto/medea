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
/// to interact in [` Room`].
impl Actor for Room {
    type Context = Context<Self>;
}

/// Connection of [`Member`].
pub trait RpcConnection: Debug + Send {
    /// Close connection.
    fn close(&self);
}

/// Message that [`Member`] has connected to [`Room`].
#[derive(Message, Debug)]
pub struct RpcConnectionEstablished {
    pub member_id: MemberID,
    pub connection: Box<dyn RpcConnection>,
}

/// Message for to get information about [`Member`] by its credentials.
#[derive(Message, Debug)]
#[rtype(result = "Option<Member>")]
pub struct GetMember {
    pub credentials: String,
}

/// Message that [`Member`] closed or lost connection.
#[derive(Message, Debug)]
pub struct RpcConnectionClosed {
    pub member_id: MemberID,
    pub reason: RpcConnectionClosedReason,
}

/// Reason closing connection of [`Member`].
#[derive(Debug)]
pub enum RpcConnectionClosedReason {
    /// [`Member`] closed connection himself.
    Disconnect,
    /// [`Member`] has lost connection.
    Idle,
}

impl Handler<GetMember> for Room {
    type Result = Option<Member>;

    /// Returns [`Member`] by its credentials if it present in [`Room`].
    fn handle(
        &mut self,
        msg: GetMember,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        self.members
            .values()
            .find(|m| m.credentials.eq(msg.credentials.as_str()))
            .map(|m| m.clone())
    }
}

impl Handler<RpcConnectionEstablished> for Room {
    type Result = ();

    /// Store connection of [`Member`] into [`Room`].
    ///
    /// If the [`Member`] already has connection, it will be closed.
    fn handle(
        &mut self,
        msg: RpcConnectionEstablished,
        _ctx: &mut Self::Context,
    ) {
        info!("RpcConnectionEstablished with member {:?}", &msg.member_id);
        if let Some(old_connection) = self.connections.remove(&msg.member_id) {
            debug!("Reconnect WsSession for member {}", msg.member_id);
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

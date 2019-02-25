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

impl Actor for Room {
    type Context = Context<Self>;
}

pub trait RpcConnection: Debug + Send {
    fn get_member_id(&self) -> Result<MemberID, Box<dyn std::error::Error>>;

    fn close(&self);
}

#[derive(Message, Debug)]
pub struct RpcConnectionEstablished {
    pub connection: Box<dyn RpcConnection>,
}

/// Message for to get information about [`Member`] by its credentials.
#[derive(Message, Debug)]
#[rtype(result = "Option<Member>")]
pub struct GetMember {
    pub credentials: String,
}

#[derive(Message, Debug)]
pub struct RpcConnectionClosed {
    pub member_id: MemberID,
    pub reason: RpcConnectionClosedReason,
}

#[derive(Debug)]
pub enum RpcConnectionClosedReason {
    Disconnect,
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

    fn handle(
        &mut self,
        msg: RpcConnectionEstablished,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let member_id = msg.connection.get_member_id();

        info!("RpcConnectionEstablished with member {:?}", &member_id);

        match member_id {
            Ok(member_id) => {
                if let Some(old_session) = self.connections.remove(&member_id) {
                    old_session.close();
                }
                self.connections.insert(member_id, msg.connection);
            }
            Err(e) => {
                error!("{:?}", e);
                msg.connection.close();
            },
        }
    }
}

impl Handler<RpcConnectionClosed> for Room {
    type Result = ();

    fn handle(
        &mut self,
        msg: RpcConnectionClosed,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        info!("RpcConnectionClosed with member {}, reason {:?}", &msg.member_id, msg.reason);
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

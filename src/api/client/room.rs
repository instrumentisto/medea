//! Room definitions and implementations.
use std::sync::{Arc, Mutex};

use actix::prelude::*;
use actix_web::ws::CloseReason;
use futures::future::Future;
use hashbrown::HashMap;

use crate::{
    api::client::{Close, WsSession},
    api::control::{Id as MemberID, Member},
    log::prelude::*,
};

/// ID of [`Room`].
pub type Id = u64;

/// Media server room with its members.
#[derive(Clone, Debug)]
pub struct Room {
    /// ID of [`Room`].
    pub id: Id,

    /// [`Member`]'s this room.
    pub members: HashMap<MemberID, Member>,

    /// [`WsSession`]s of [`Member`]'s this room.
    pub sessions: HashMap<MemberID, Addr<WsSession>>,
}

/// Message for to get information about [`Member`] by its credentials.
#[derive(Message)]
#[rtype(result = "Option<Member>")]
pub struct GetMember(pub String);

impl Handler<GetMember> for Room {
    type Result = Option<Member>;

    /// Returns [`Member`] by its credentials if it present in [`Room`].
    fn handle(
        &mut self,
        credentials: GetMember,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        debug!("retrieve member by credentials: {}", credentials.0);
        self.members
            .values()
            .find(|m| m.credentials.eq(credentials.0.as_str()))
            .map(|m| m.clone())
    }
}

/// Message from [`WsSession`] signaling what [`Member`] connected.
#[derive(Message)]
pub struct JoinMember(pub MemberID, pub Addr<WsSession>);

impl Handler<JoinMember> for Room {
    type Result = ();

    /// Stores [`WsSession`] of [`Member`] into [`Room`].
    ///
    /// If [`Member`] is reconnected, close and stop old [`WsSession`]
    /// before store current [`WsSession`] in [`Room`].
    fn handle(&mut self, msg: JoinMember, _ctx: &mut Self::Context) {
        debug!("join member: {}", msg.0);
        if let Some(old_session) = self.sessions.remove(&msg.0) {
            let _ = old_session.send(Close(None)).wait();
        }
        self.sessions.insert(msg.0, msg.1);
    }
}

/// Message from [`WsSession`] signaling what [`Member`] closed connection
/// or become idle.
#[derive(Message)]
pub struct LeaveMember(pub MemberID, pub Option<CloseReason>);

impl Handler<LeaveMember> for Room {
    type Result = ();

    /// Remove and close [`WsSession`] from [`Room`].
    fn handle(&mut self, msg: LeaveMember, _ctx: &mut Self::Context) {
        debug!("leave member: {}", msg.0);
        if let Some(session) = self.sessions.remove(&msg.0) {
            session.do_send(Close(msg.1))
        }
    }
}

impl Actor for Room {
    type Context = Context<Self>;
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
        debug!("retrieve room by id: {}", id);
        let rooms = self.rooms.lock().unwrap();
        rooms.get(&id).map(|r| r.clone())
    }
}

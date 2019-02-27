//! Room definitions and implementations.
use std::sync::{Arc, Mutex};

use actix::prelude::*;
use hashbrown::HashMap;
use serde::{Deserialize, Serialize};

use crate::{
    api::control::{Id as MemberID, Member},
    log::prelude::*,
    media::{Id as PeerID, Peer, PeerMachine},
};
use std::fmt::Debug;

use failure::Fail;

#[derive(Fail, Debug)]
pub enum RoomError {
    #[fail(display = "Unknown peer {}", _0)]
    UnknownPeer(PeerID),
    #[fail(display = "Unmatched state of peer {}", _0)]
    UnmatchedState(PeerID),
}

#[derive(Debug, Deserialize, Serialize, Message)]
#[rtype(result = "Result<(), RoomError>")]
pub enum Command {
    MakeSdpOffer { peer_id: PeerID, sdp_offer: String },
    MakeSdpAnswer,
}

#[derive(Debug, Deserialize, Serialize, Message)]
pub enum Event {
    PeerCreated {
        peer: PeerID,
        sdp_offer: Option<String>,
    },
}

/// ID of [`Room`].
pub type Id = u64;

/// Media server room with its members.
#[derive(Debug)]
pub struct Room {
    /// ID of [`Room`].
    id: Id,

    /// [`Member`]'s this room.
    members: HashMap<MemberID, Member>,

    /// Connections of [`Member`]'s this room.
    connections: HashMap<MemberID, Box<dyn RpcConnection>>,

    /// [`Peer`]s of [`Member`]'s this room.
    peers: HashMap<PeerID, PeerMachine>,
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

impl Room {
    pub fn new(id: Id, members: HashMap<MemberID, Member>) -> Self {
        Room {
            id,
            members,
            connections: HashMap::new(),
            peers: HashMap::new(),
        }
    }

    fn start_pipeline(&mut self, members: (MemberID, MemberID)) {
        let peer_responder_id = 2;
        let peer_responder =
            PeerMachine::New(Peer::new(peer_responder_id, members.1));
        let peer_caller_id = 1;
        let peer_caller = PeerMachine::WaitLocalSDP(
            Peer::new(peer_caller_id, members.0).start(peer_responder_id),
        );
        let event = Event::PeerCreated {
            peer: peer_caller_id,
            sdp_offer: None,
        };
        let caller_session = self.sessions.get(&members.0).unwrap();
        caller_session.do_send(event);
        self.peers.insert(peer_caller_id, peer_caller);
        self.peers.insert(peer_responder_id, peer_responder);
    }
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

/// [`RpcConnection`] close reasons.
#[derive(Debug)]
pub enum RpcConnectionClosedReason {
    /// [`RpcConnection`] gracefully disconnected from server.
    Disconnect,
    /// [`RpcConnection`] was considered idle.
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
            reconnected = true;
            debug!("Reconnect WsSession for member {}", msg.member_id);
            old_connection.close();
        }
        self.connections.insert(msg.member_id, msg.connection);
        if !reconnected && self.connections.len() > 1 {
            self.start_pipeline((1, 2));
        }
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

impl Handler<Command> for Room {
    type Result = Result<(), RoomError>;

    fn handle(
        &mut self,
        command: Command,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        debug!("receive command: {:?}", command);
        match command {
            Command::MakeSdpOffer { peer_id, sdp_offer } => {
                let peer_caller = self
                    .peers
                    .remove(&peer_id)
                    .ok_or(RoomError::UnknownPeer(peer_id))?;
                let (new_peer_caller, opponent_peer_id) = match peer_caller {
                    PeerMachine::WaitLocalSDP(peer) => Ok((
                        PeerMachine::WaitRemoteSDP(
                            peer.set_local_sdp(sdp_offer),
                        ),
                        peer.context.opponent_peer_id.unwrap(),
                    )),
                    _ => Err(RoomError::UnmatchedState(peer_id)),
                }?;
                self.peers.insert(peer_id, new_peer_caller);
                let peer_responder = self
                    .peers
                    .remove(&opponent_peer_id)
                    .ok_or(RoomError::UnknownPeer(opponent_peer_id))?;
                let new_peer_responder = match peer_responder {
                    PeerMachine::New(peer) => {
                        Ok(PeerMachine::WaitLocalHaveRemote(
                            peer.set_remote_sdp(sdp_offer),
                        ))
                    }
                    _ => Err(RoomError::UnmatchedState(opponent_peer_id)),
                }?;
                self.peers.insert(opponent_peer_id, new_peer_responder);
                Ok(())
            }
            _ => unimplemented!(),
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

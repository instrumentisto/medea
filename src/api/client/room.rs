//! Room definitions and implementations.
use std::sync::{Arc, Mutex};

use actix::prelude::*;
use actix_web::ws::CloseReason;
use futures::future::Future;
use hashbrown::HashMap;
use serde::{Deserialize, Serialize};

use crate::{
    api::client::{Close, WsSession},
    api::control::{Id as MemberID, Member},
    log::prelude::*,
    media::{Id as PeerID, Peer, PeerMachine},
};

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
#[derive(Clone, Debug)]
pub struct Room {
    /// ID of [`Room`].
    id: Id,

    /// [`Member`]'s this room.
    members: HashMap<MemberID, Member>,

    /// [`WsSession`]s of [`Member`]'s this room.
    sessions: HashMap<MemberID, Addr<WsSession>>,

    /// [`Peer`]s of [`Member`]'s this room.
    peers: HashMap<PeerID, PeerMachine>,
}

impl Room {
    pub fn new(id: Id, members: HashMap<MemberID, Member>) -> Self {
        Room {
            id,
            members,
            sessions: HashMap::new(),
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

impl Actor for Room {
    type Context = Context<Self>;
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
        let mut reconnected = false;
        if let Some(old_session) = self.sessions.remove(&msg.0) {
            reconnected = true;
            let _ = old_session.send(Close(None)).wait();
        }
        self.sessions.insert(msg.0, msg.1);
        if !reconnected && self.sessions.len() > 1 {
            self.start_pipeline((1, 2));
        }
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
        debug!("retrieve room by id: {}", id);
        let rooms = self.rooms.lock().unwrap();
        rooms.get(&id).map(|r| r.clone())
    }
}

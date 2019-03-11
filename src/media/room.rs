//! Room definitions and implementations.
use std::sync::{Arc, Mutex};

use actix::prelude::*;
use hashbrown::HashMap;
use serde::{Deserialize, Serialize};

use crate::{
    api::control::{Id as MemberID, Member},
    log::prelude::*,
    media::{
        peer::{Event, Id as PeerID, Peer, PeerMachine},
        track::{
            AudioSettings, DirectionalTrack, Track, TrackMediaType,
            VideoSettings,
        },
    },
};
use std::fmt::Debug;

use failure::Fail;

#[derive(Fail, Debug)]
pub enum RoomError {
    #[fail(display = "Member without peer {}", _0)]
    MemberWithoutPeer(MemberID),
    #[fail(display = "Invalid connection of member {}", _0)]
    InvalidConnection(MemberID),
    #[fail(display = "Unknown peer {}", _0)]
    UnknownPeer(PeerID),
    #[fail(display = "Peer dont have opponent {}", _0)]
    NoOpponentPeer(PeerID),
    #[fail(display = "Unmatched state of peer {}", _0)]
    UnmatchedState(PeerID),
}

/// WebSocket message from Web Client to Media Server.
#[derive(Debug, Deserialize, Serialize, Message)]
#[rtype(result = "Result<bool, RoomError>")]
pub enum Command {
    /// Web Client sends SDP Offer.
    MakeSdpOffer {
        peer_id: PeerID,
        sdp_offer: String,
    },
    /// Web Client sends SDP Answer.
    MakeSdpAnswer {
        peer_id: PeerID,
        sdp_answer: String,
    },

    SetIceCandidate {
        peer_id: PeerID,
        candidate: String,
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

    /// Peers of [`Member`]'s this room.
    member_peers: HashMap<MemberID, PeerID>,

    /// Relations peer to peer.
    peer_to_peer: HashMap<PeerID, PeerID>,

    /// [`Peer`]s of [`Member`]'s this room.
    peers: HashMap<PeerID, PeerMachine>,

    peer_index: u64,
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

    /// Send event.
    fn send_event(&self, event: Event);
}

/// Message that [`Member`] has connected to [`Room`].
#[derive(Message, Debug)]
#[rtype(result = "Result<(), RoomError>")]
pub struct RpcConnectionEstablished {
    pub member_id: MemberID,
    pub connection: Box<dyn RpcConnection>,
}

impl Room {
    /// Create new instance of [`Room`].
    pub fn new(id: Id, members: HashMap<MemberID, Member>) -> Self {
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
    fn next_peer_id(&mut self) -> PeerID {
        let id = self.peer_index;
        self.peer_index += 1;
        id
    }

    fn remove_peer_by_id(
        &mut self,
        peer_id: &PeerID,
    ) -> Result<PeerMachine, RoomError> {
        self.peers
            .remove(peer_id)
            .ok_or(RoomError::UnknownPeer(*peer_id))
    }

    fn get_peer_by_id(
        &self,
        peer_id: &PeerID,
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
        caller: MemberID,
        responder: MemberID,
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
        from_peer_id: PeerID,
        from_peer: PeerMachine,
        sdp_offer: String,
    ) -> Result<bool, RoomError> {
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
        Ok(true)
    }

    fn handle_make_sdp_answer(
        &mut self,
        from_peer_id: PeerID,
        from_peer: PeerMachine,
        sdp_answer: String,
    ) -> Result<bool, RoomError> {
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

        Ok(true)
    }

    fn handle_set_ice_candidate(
        &self,
        from_peer_id: PeerID,
        from_peer: &PeerMachine,
        candidate: String,
    ) -> Result<bool, RoomError> {

        fn send_candidate_to_remote(
            room: &Room,
            from_peer_id: PeerID,
            candidate: String,
        ) -> Result<bool, RoomError> {
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

            Ok(true)
        };

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
    type Result = Result<(), RoomError>;

    /// Store connection of [`Member`] into [`Room`].
    ///
    /// If the [`Member`] already has connection, it will be closed.
    fn handle(
        &mut self,
        msg: RpcConnectionEstablished,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        info!("RpcConnectionEstablished with member {:?}", &msg.member_id);
        let mut reconnected = false;
        if let Some(old_connection) = self.connections.remove(&msg.member_id) {
            reconnected = true;
            debug!("Reconnect WsSession for member {}", msg.member_id);
            old_connection.close();
        }
        self.connections.insert(msg.member_id, msg.connection);
        if !reconnected {
            let peer_id = self.next_peer_id();
            let peer = PeerMachine::New(Peer::new(peer_id, msg.member_id));
            self.peers.insert(peer_id, peer);
            self.member_peers.insert(msg.member_id, peer_id);

            if self.connections.len() > 1 {
                self.start_pipeline(1, 2)?;
            }
        }
        Ok(())
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
    type Result = Result<bool, RoomError>;

    /// Receives [`Command`] from Web client and changes state of interconnected
    /// [`Peer`]s.
    fn handle(
        &mut self,
        command: Command,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        debug!("receive command: {:?}", command);
        match command {
            Command::MakeSdpOffer { peer_id, sdp_offer } => {
                let from_peer = self.remove_peer_by_id(&peer_id)?;
                self.handle_make_sdp_offer(peer_id, from_peer, sdp_offer)
            }
            Command::MakeSdpAnswer {
                peer_id,
                sdp_answer,
            } => {
                let from_peer = self.remove_peer_by_id(&peer_id)?;
                self.handle_make_sdp_answer(peer_id, from_peer, sdp_answer)
            }
            Command::SetIceCandidate { peer_id, candidate } => {
                let from_peer = self.get_peer_by_id(&peer_id)?;
                self.handle_set_ice_candidate(peer_id, from_peer, candidate)
            }
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

#[cfg(test)]
mod test {
    use std::time::{Duration, Instant};

    use futures::future::{result, Future};
    use tokio::timer::Delay;

    use super::*;

    #[derive(Debug, Clone)]
    struct TestConnection {
        pub member_id: MemberID,
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
            }
        }
    }

    impl RpcConnection for Addr<TestConnection> {
        fn close(&self) {}

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

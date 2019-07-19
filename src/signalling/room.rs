//! Room definitions and implementations. Room is responsible for media
//! connection establishment between concrete [`Member`]s.

use std::{collections::HashMap as StdHashMap, sync::Arc, time::Duration};

use actix::{
    fut::wrap_future, Actor, ActorFuture, AsyncContext, Context, Handler,
    Message, ResponseActFuture, WrapFuture as _,
};
use failure::Fail;
use futures::future;
use hashbrown::HashMap;
use medea_client_api_proto::{Command, Event, IceCandidate};

use crate::{
    api::{
        client::rpc_connection::{
            AuthorizationError, Authorize, ClosedReason, CommandMessage,
            RpcConnectionClosed, RpcConnectionEstablished,
        },
        control::{MemberId, RoomId, RoomSpec, TryFromElementError},
    },
    log::prelude::*,
    media::{
        New, Peer, PeerError, PeerId, PeerStateMachine, WaitLocalHaveRemote,
        WaitLocalSdp, WaitRemoteSdp,
    },
    shutdown::ShutdownGracefully,
    signalling::{
        elements::{
            endpoints::webrtc::{WebRtcPlayEndpoint, WebRtcPublishEndpoint},
            Member, MembersLoadError,
        },
        participants::ParticipantService,
        peers::PeerRepository,
    },
    turn::BoxedTurnAuthService,
};

/// Ergonomic type alias for using [`ActorFuture`] for [`Room`].
pub type ActFuture<I, E> =
    Box<dyn ActorFuture<Actor = Room, Item = I, Error = E>>;

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Fail)]
pub enum RoomError {
    #[fail(display = "Couldn't find Peer with [id = {}]", _0)]
    PeerNotFound(PeerId),
    #[fail(display = "Couldn't find Member with [id = {}]", _0)]
    MemberNotFound(MemberId),
    #[fail(display = "Member [id = {}] does not have Turn credentials", _0)]
    NoTurnCredentials(MemberId),
    #[fail(display = "Couldn't find RpcConnection with Member [id = {}]", _0)]
    ConnectionNotExists(MemberId),
    #[fail(display = "Unable to send event to Member [id = {}]", _0)]
    UnableToSendEvent(MemberId),
    #[fail(display = "PeerError: {}", _0)]
    PeerError(PeerError),
    #[fail(display = "Generic room error {}", _0)]
    BadRoomSpec(String),
    #[fail(display = "Turn service error: {}", _0)]
    TurnServiceError(String),
    #[fail(display = "Client error:{}", _0)]
    ClientError(String),
}

impl From<PeerError> for RoomError {
    fn from(err: PeerError) -> Self {
        RoomError::PeerError(err)
    }
}

impl From<TryFromElementError> for RoomError {
    fn from(err: TryFromElementError) -> Self {
        RoomError::BadRoomSpec(format!(
            "Element located in wrong place. {}",
            err
        ))
    }
}

impl From<MembersLoadError> for RoomError {
    fn from(err: MembersLoadError) -> Self {
        RoomError::BadRoomSpec(format!(
            "Error while loading room spec. {}",
            err
        ))
    }
}

/// Possible states of [`Room`].
#[derive(Debug)]
enum State {
    /// [`Room`] has been started and is operating at the moment.
    Started,
    /// [`Room`] is stopping at the moment.
    Stopping,
    /// [`Room`] is stopped and can be removed.
    Stopped,
}

/// Media server room with its [`Member`]s.
#[derive(Debug)]
pub struct Room {
    id: RoomId,

    /// [`RpcConnection`]s of [`Member`]s in this [`Room`].
    pub members: ParticipantService,

    /// [`Peer`]s of [`Member`]s in this [`Room`].
    peers: PeerRepository,

    /// Current state of this [`Room`].
    state: State,
}

impl Room {
    /// Create new instance of [`Room`].
    ///
    /// Returns [`RoomError::BadRoomSpec`] when error while [`Element`]
    /// transformation happens.
    pub fn new(
        room_spec: &RoomSpec,
        reconnect_timeout: Duration,
        turn: Arc<BoxedTurnAuthService>,
    ) -> Result<Self, RoomError> {
        Ok(Self {
            id: room_spec.id().clone(),
            peers: PeerRepository::from(HashMap::new()),
            members: ParticipantService::new(
                room_spec,
                reconnect_timeout,
                turn,
            )?,
            state: State::Started,
        })
    }

    /// Returns [`RoomId`] of this [`Room`].
    pub fn get_id(&self) -> RoomId {
        self.id.clone()
    }

    /// Sends [`Event::PeerCreated`] to one of specified [`Peer`]s based on
    /// which of them has any outbound tracks. That [`Peer`] state will be
    /// changed to [`WaitLocalSdp`] state. Both provided peers must be in
    /// [`New`] state. At least one of provided peers must have outbound
    /// tracks.
    fn send_peer_created(
        &mut self,
        peer1_id: PeerId,
        peer2_id: PeerId,
    ) -> Result<ActFuture<(), RoomError>, RoomError> {
        let peer1: Peer<New> = self.peers.take_inner_peer(peer1_id)?;
        let peer2: Peer<New> = self.peers.take_inner_peer(peer2_id)?;

        // decide which peer is sender
        let (sender, receiver) = if peer1.is_sender() {
            (peer1, peer2)
        } else if peer2.is_sender() {
            (peer2, peer1)
        } else {
            self.peers.add_peer(peer1);
            self.peers.add_peer(peer2);
            return Err(RoomError::BadRoomSpec(format!(
                "Error while trying to connect Peer [id = {}] and Peer [id = \
                 {}] cause neither of peers are senders",
                peer1_id, peer2_id
            )));
        };
        self.peers.add_peer(receiver);

        let sender = sender.start();
        let member_id = sender.member_id();
        let ice_servers = self
            .members
            .get_member_by_id(&member_id)
            .ok_or_else(|| RoomError::MemberNotFound(member_id.clone()))?
            .servers_list()
            .ok_or_else(|| RoomError::NoTurnCredentials(member_id.clone()))?;
        let peer_created = Event::PeerCreated {
            peer_id: sender.id(),
            sdp_offer: None,
            tracks: sender.tracks(),
            ice_servers,
        };
        self.peers.add_peer(sender);
        Ok(Box::new(wrap_future(
            self.members.send_event_to_member(member_id, peer_created),
        )))
    }

    /// Sends [`Event::PeersRemoved`] to [`Member`].
    fn send_peers_removed(
        &mut self,
        member_id: MemberId,
        peers: Vec<PeerId>,
    ) -> ActFuture<(), RoomError> {
        Box::new(wrap_future(self.members.send_event_to_member(
            member_id,
            Event::PeersRemoved { peer_ids: peers },
        )))
    }

    /// Sends [`Event::PeerCreated`] to provided [`Peer`] partner. Provided
    /// [`Peer`] state must be [`WaitLocalSdp`] and will be changed to
    /// [`WaitRemoteSdp`], partners [`Peer`] state must be [`New`] and will be
    /// changed to [`WaitLocalHaveRemote`].
    fn handle_make_sdp_offer(
        &mut self,
        from_peer_id: PeerId,
        sdp_offer: String,
        mids: StdHashMap<u64, String>,
    ) -> Result<ActFuture<(), RoomError>, RoomError> {
        let mut from_peer: Peer<WaitLocalSdp> =
            self.peers.take_inner_peer(from_peer_id)?;
        from_peer.set_mids(mids)?;

        let to_peer_id = from_peer.partner_peer_id();
        let to_peer: Peer<New> = self.peers.take_inner_peer(to_peer_id)?;

        let from_peer = from_peer.set_local_sdp(sdp_offer.clone());
        let to_peer = to_peer.set_remote_sdp(sdp_offer.clone());

        let to_member_id = to_peer.member_id();
        let ice_servers = self
            .members
            .get_member_by_id(&to_member_id)
            .ok_or_else(|| RoomError::MemberNotFound(to_member_id.clone()))?
            .servers_list()
            .ok_or_else(|| {
                RoomError::NoTurnCredentials(to_member_id.clone())
            })?;

        let event = Event::PeerCreated {
            peer_id: to_peer.id(),
            sdp_offer: Some(sdp_offer),
            tracks: to_peer.tracks(),
            ice_servers,
        };

        self.peers.add_peer(from_peer);
        self.peers.add_peer(to_peer);

        Ok(Box::new(wrap_future(
            self.members.send_event_to_member(to_member_id, event),
        )))
    }

    /// Sends [`Event::SdpAnswerMade`] to provided [`Peer`] partner. Provided
    /// [`Peer`] state must be [`WaitLocalHaveRemote`] and will be changed to
    /// [`Stable`], partners [`Peer`] state must be [`WaitRemoteSdp`] and will
    /// be changed to [`Stable`].
    fn handle_make_sdp_answer(
        &mut self,
        from_peer_id: PeerId,
        sdp_answer: String,
    ) -> Result<ActFuture<(), RoomError>, RoomError> {
        let from_peer: Peer<WaitLocalHaveRemote> =
            self.peers.take_inner_peer(from_peer_id)?;

        let to_peer_id = from_peer.partner_peer_id();
        let to_peer: Peer<WaitRemoteSdp> =
            self.peers.take_inner_peer(to_peer_id)?;

        let from_peer = from_peer.set_local_sdp(sdp_answer.clone());
        let to_peer = to_peer.set_remote_sdp(&sdp_answer);

        let to_member_id = to_peer.member_id();
        let event = Event::SdpAnswerMade {
            peer_id: to_peer_id,
            sdp_answer,
        };

        self.peers.add_peer(from_peer);
        self.peers.add_peer(to_peer);

        Ok(Box::new(wrap_future(
            self.members.send_event_to_member(to_member_id, event),
        )))
    }

    /// Sends [`Event::IceCandidateDiscovered`] to provided [`Peer`] partner.
    /// Both [`Peer`]s may have any state except [`New`].
    fn handle_set_ice_candidate(
        &mut self,
        from_peer_id: PeerId,
        candidate: IceCandidate,
    ) -> Result<ActFuture<(), RoomError>, RoomError> {
        let from_peer = self.peers.get_peer(from_peer_id)?;
        if let PeerStateMachine::New(_) = from_peer {
            return Err(PeerError::WrongState(
                from_peer_id,
                "Not New",
                format!("{}", from_peer),
            )
            .into());
        }

        let to_peer_id = from_peer.partner_peer_id();
        let to_peer = self.peers.get_peer(to_peer_id)?;
        if let PeerStateMachine::New(_) = to_peer {
            return Err(PeerError::WrongState(
                to_peer_id,
                "Not New",
                format!("{}", to_peer),
            )
            .into());
        }

        let to_member_id = to_peer.member_id();
        let event = Event::IceCandidateDiscovered {
            peer_id: to_peer_id,
            candidate,
        };

        Ok(Box::new(wrap_future(
            self.members.send_event_to_member(to_member_id, event),
        )))
    }

    /// Create [`Peer`] for endpoints if [`Peer`] between endpoint's members
    /// not exist.
    ///
    /// Add `send` track to source member's [`Peer`] and `recv` to
    /// sink member's [`Peer`].
    ///
    /// Returns [`PeerId`]s of newly created [`Peer`] if some created.
    ///
    /// __This will panic if provide endpoints with already interconnected
    /// [`Peer`]s!__
    fn connect_endpoints(
        &mut self,
        src: &WebRtcPublishEndpoint,
        sink: &WebRtcPlayEndpoint,
    ) -> Option<(PeerId, PeerId)> {
        let src_owner = src.owner();
        let sink_owner = sink.owner();

        if let Some((src_peer_id, sink_peer_id)) = self
            .peers
            .get_peer_by_members_ids(&src_owner.id(), &sink_owner.id())
        {
            // TODO: when dynamic patching of [`Room`] will be done then we need
            //       rewrite this code to updating [`Peer`]s in not
            //       [`Peer<New>`] state.
            let mut src_peer: Peer<New> =
                self.peers.take_inner_peer(src_peer_id).unwrap();
            let mut sink_peer: Peer<New> =
                self.peers.take_inner_peer(sink_peer_id).unwrap();

            src_peer
                .add_publisher(&mut sink_peer, self.peers.get_tracks_counter());

            src.add_peer_id(src_peer_id);
            sink.set_peer_id(sink_peer_id);

            self.peers.add_peer(src_peer);
            self.peers.add_peer(sink_peer);
        } else {
            let (mut src_peer, mut sink_peer) =
                self.peers.create_peers(&src_owner, &sink_owner);

            src_peer
                .add_publisher(&mut sink_peer, self.peers.get_tracks_counter());

            src.add_peer_id(src_peer.id());
            sink.set_peer_id(sink_peer.id());

            let src_peer_id = src_peer.id();
            let sink_peer_id = sink_peer.id();

            self.peers.add_peer(src_peer);
            self.peers.add_peer(sink_peer);

            return Some((src_peer_id, sink_peer_id));
        };

        None
    }

    /// Create and interconnect all [`Peer`]s between connected [`Member`]
    /// and all available at this moment [`Member`].
    ///
    /// Availability is determines by checking [`RpcConnection`] of all
    /// [`Member`]s from [`WebRtcPlayEndpoint`]s and from receivers of
    /// connected [`Member`].
    fn init_member_connections(
        &mut self,
        member: &Member,
        ctx: &mut <Self as Actor>::Context,
    ) {
        let mut created_peers: Vec<(PeerId, PeerId)> = Vec::new();

        // Create all connected publish endpoints.
        for (_, publisher) in member.srcs() {
            for receiver in publisher.sinks() {
                let receiver_owner = receiver.owner();

                if receiver.peer_id().is_none()
                    && self.members.member_has_connection(&receiver_owner.id())
                {
                    if let Some(p) =
                        self.connect_endpoints(&publisher, &receiver)
                    {
                        created_peers.push(p)
                    }
                }
            }
        }

        // Create all connected play's receivers peers.
        for (_, receiver) in member.sinks() {
            let publisher = receiver.src();

            if receiver.peer_id().is_none()
                && self.members.member_has_connection(&publisher.owner().id())
            {
                if let Some(p) = self.connect_endpoints(&publisher, &receiver) {
                    created_peers.push(p);
                }
            }
        }

        for (first_peer_id, second_peer_id) in created_peers {
            self.connect_peers(ctx, first_peer_id, second_peer_id);
        }
    }

    /// Check state of interconnected [`Peer`]s and sends [`Event`] about
    /// [`Peer`] created to remote [`Member`].
    fn connect_peers(
        &mut self,
        ctx: &mut Context<Self>,
        first_peer: PeerId,
        second_peer: PeerId,
    ) {
        let fut: ActFuture<(), ()> = match self
            .send_peer_created(first_peer, second_peer)
        {
            Ok(res) => {
                Box::new(res.then(|res, room, ctx| -> ActFuture<(), ()> {
                    if res.is_ok() {
                        return Box::new(future::ok(()).into_actor(room));
                    }
                    error!(
                        "Failed handle command, because {}. Room will be \
                         stopped.",
                        res.unwrap_err(),
                    );
                    room.close_gracefully(ctx)
                }))
            }
            Err(err) => {
                error!(
                    "Failed handle command, because {}. Room will be stopped.",
                    err
                );
                self.close_gracefully(ctx)
            }
        };

        ctx.spawn(fut);
    }

    /// Closes [`Room`] gracefully, by dropping all the connections and moving
    /// into [`State::Stopped`].
    fn close_gracefully(
        &mut self,
        ctx: &mut Context<Self>,
    ) -> ResponseActFuture<Self, (), ()> {
        info!("Closing Room [id = {:?}]", self.id);
        self.state = State::Stopping;

        Box::new(
            self.members
                .drop_connections(ctx)
                .into_actor(self)
                .map(move |_, room: &mut Self, _| {
                    room.state = State::Stopped;
                })
                .map_err(move |_, room, _| {
                    error!("Error closing room {:?}", room.id);
                }),
        )
    }
}

/// [`Actor`] implementation that provides an ergonomic way
/// to interact with [`Room`].
impl Actor for Room {
    type Context = Context<Self>;
}

impl Handler<Authorize> for Room {
    type Result = Result<(), AuthorizationError>;

    /// Responses with `Ok` if `RpcConnection` is authorized, otherwise `Err`s.
    fn handle(
        &mut self,
        msg: Authorize,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        self.members
            .get_member_by_id_and_credentials(&msg.member_id, &msg.credentials)
            .map(|_| ())
    }
}

/// Signal of removing [`Member`]'s [`Peer`]s.
#[derive(Debug, Message)]
#[rtype(result = "Result<(), ()>")]
pub struct PeersRemoved {
    pub peers_id: Vec<PeerId>,
    pub member_id: MemberId,
}

impl Handler<PeersRemoved> for Room {
    type Result = ActFuture<(), ()>;

    /// Send [`Event::PeersRemoved`] to remote [`Member`].
    ///
    /// Delete all removed [`PeerId`]s from all [`Member`]'s
    /// endpoints.
    #[allow(clippy::single_match_else)]
    fn handle(
        &mut self,
        msg: PeersRemoved,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        info!(
            "Peers {:?} removed for member '{}'.",
            msg.peers_id, msg.member_id
        );
        if let Some(member) = self.members.get_member_by_id(&msg.member_id) {
            member.peers_removed(&msg.peers_id);
        } else {
            error!(
                "Participant with id {} for which received \
                 Event::PeersRemoved not found. Closing room.",
                msg.member_id
            );

            return Box::new(self.close_gracefully(ctx));
        }

        Box::new(self.send_peers_removed(msg.member_id, msg.peers_id).then(
            |err, room, ctx: &mut Context<Self>| {
                if let Err(e) = err {
                    match e {
                        RoomError::ConnectionNotExists(_) => {
                            Box::new(future::ok(()).into_actor(room))
                        }
                        _ => {
                            error!(
                                "Failed PeersEvent command, because {}. Room \
                                 will be stopped.",
                                e
                            );
                            room.close_gracefully(ctx)
                        }
                    }
                } else {
                    Box::new(future::ok(()).into_actor(room))
                }
            },
        ))
    }
}

impl Handler<CommandMessage> for Room {
    type Result = ActFuture<(), ()>;

    /// Receives [`Command`] from Web client and passes it to corresponding
    /// handlers. Will emit [`CloseRoom`] on any error.
    fn handle(
        &mut self,
        msg: CommandMessage,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        let result = match msg.into() {
            Command::MakeSdpOffer {
                peer_id,
                sdp_offer,
                mids,
            } => self.handle_make_sdp_offer(peer_id, sdp_offer, mids),
            Command::MakeSdpAnswer {
                peer_id,
                sdp_answer,
            } => self.handle_make_sdp_answer(peer_id, sdp_answer),
            Command::SetIceCandidate { peer_id, candidate } => {
                self.handle_set_ice_candidate(peer_id, candidate)
            }
        };

        match result {
            Ok(res) => {
                Box::new(res.then(|res, room, ctx| -> ActFuture<(), ()> {
                    if res.is_ok() {
                        return Box::new(future::ok(()).into_actor(room));
                    }
                    error!(
                        "Failed handle command, because {}. Room will be \
                         stopped.",
                        res.unwrap_err(),
                    );
                    room.close_gracefully(ctx)
                }))
            }
            Err(err) => {
                error!(
                    "Failed handle command, because {}. Room will be stopped.",
                    err
                );
                self.close_gracefully(ctx)
            }
        }
    }
}

impl Handler<RpcConnectionEstablished> for Room {
    type Result = ActFuture<(), ()>;

    /// Saves new [`RpcConnection`] in [`ParticipantService`], initiates media
    /// establishment between members.
    /// Create and interconnect all available [`Member`]'s [`Peer`]s.
    fn handle(
        &mut self,
        msg: RpcConnectionEstablished,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        info!("RpcConnectionEstablished for member {}", &msg.member_id);

        let fut = self
            .members
            .connection_established(ctx, msg.member_id, msg.connection)
            .map_err(|err, _, _| {
                error!("RpcConnectionEstablished error {:?}", err)
            })
            .map(|member, room, ctx| {
                room.init_member_connections(&member, ctx);
            });
        Box::new(fut)
    }
}

impl Handler<ShutdownGracefully> for Room {
    type Result = ResponseActFuture<Self, (), ()>;

    fn handle(
        &mut self,
        _: ShutdownGracefully,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        info!(
            "Room: {:?} received ShutdownGracefully message so shutting down",
            self.id
        );
        self.close_gracefully(ctx)
    }
}

impl Handler<RpcConnectionClosed> for Room {
    type Result = ();

    /// Passes message to [`ParticipantService`] to cleanup stored connections.
    /// Remove all related for disconnected [`Member`] [`Peer`]s.
    fn handle(&mut self, msg: RpcConnectionClosed, ctx: &mut Self::Context) {
        info!(
            "RpcConnectionClosed for member {}, reason {:?}",
            msg.member_id, msg.reason
        );

        if let ClosedReason::Closed = msg.reason {
            self.peers.connection_closed(&msg.member_id, ctx);
        }

        self.members
            .connection_closed(msg.member_id, &msg.reason, ctx);
    }
}

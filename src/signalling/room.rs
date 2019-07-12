//! Room definitions and implementations. Room is responsible for media
//! connection establishment between concrete [`Member`]s.

use std::{
    collections::HashMap as StdHashMap, rc::Rc, sync::Arc, time::Duration,
};

use actix::{
    fut::wrap_future, Actor, ActorFuture, AsyncContext, Context, Handler,
    Message,
};
use failure::Fail;
use futures::future;
use hashbrown::{HashMap, HashSet};
use medea_client_api_proto::{Command, Event, IceCandidate};

use crate::{
    api::{
        client::rpc_connection::{
            AuthorizationError, Authorize, ClosedReason, CommandMessage,
            RpcConnectionClosed, RpcConnectionEstablished,
        },
        control::{
            grpc::protos::control::{
                Element as ElementProto, Member_Element, Room as RoomProto,
                Room_Element,
            },
            local_uri::LocalUri,
            room::RoomSpec,
            Endpoint as EndpointSpec, MemberId, MemberSpec, RoomId,
            TryFromElementError, WebRtcPlayId, WebRtcPublishId,
        },
        error_codes::ErrorCode,
    },
    log::prelude::*,
    media::{
        New, Peer, PeerId, PeerStateError, PeerStateMachine,
        WaitLocalHaveRemote, WaitLocalSdp, WaitRemoteSdp,
    },
    signalling::{
        elements::{
            endpoints::webrtc::{WebRtcPlayEndpoint, WebRtcPublishEndpoint},
            member::MemberError,
            Member, MembersLoadError,
        },
        participants::{ParticipantService, ParticipantServiceErr},
        peers::PeerRepository,
    },
    turn::TurnAuthService,
};

/// Ergonomic type alias for using [`ActorFuture`] for [`Room`].
pub type ActFuture<I, E> =
    Box<dyn ActorFuture<Actor = Room, Item = I, Error = E>>;

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Fail)]
pub enum RoomError {
    #[fail(display = "Couldn't find Peer with [id = {}]", _0)]
    PeerNotFound(PeerId),
    #[fail(display = "{}", _0)]
    MemberError(MemberError),
    #[fail(display = "Member [id = {}] does not have Turn credentials", _0)]
    NoTurnCredentials(MemberId),
    #[fail(display = "Couldn't find RpcConnection with Member [id = {}]", _0)]
    ConnectionNotExists(MemberId),
    #[fail(display = "Unable to send event to Member [id = {}]", _0)]
    UnableToSendEvent(MemberId),
    #[fail(display = "PeerError: {}", _0)]
    PeerStateError(PeerStateError),
    #[fail(display = "{}", _0)]
    MembersLoadError(MembersLoadError),
    #[fail(display = "{}", _0)]
    TryFromElementError(TryFromElementError),
    #[fail(display = "Generic room error: {}", _0)]
    BadRoomSpec(String),
    #[fail(display = "Turn service error: {}", _0)]
    TurnServiceError(String),
    #[fail(display = "{}", _0)]
    ParticipantServiceErr(ParticipantServiceErr),
}

impl From<PeerStateError> for RoomError {
    fn from(err: PeerStateError) -> Self {
        RoomError::PeerStateError(err)
    }
}

impl From<TryFromElementError> for RoomError {
    fn from(err: TryFromElementError) -> Self {
        RoomError::TryFromElementError(err)
    }
}

impl From<MembersLoadError> for RoomError {
    fn from(err: MembersLoadError) -> Self {
        RoomError::MembersLoadError(err)
    }
}

impl From<ParticipantServiceErr> for RoomError {
    fn from(err: ParticipantServiceErr) -> Self {
        RoomError::ParticipantServiceErr(err)
    }
}

impl From<MemberError> for RoomError {
    fn from(err: MemberError) -> Self {
        RoomError::MemberError(err)
    }
}

impl Into<ErrorCode> for RoomError {
    fn into(self) -> ErrorCode {
        match self {
            RoomError::MemberError(e) => e.into(),
            RoomError::MembersLoadError(e) => e.into(),
            RoomError::ParticipantServiceErr(e) => e.into(),
            _ => ErrorCode::UnknownError(self.to_string()),
        }
    }
}

/// Media server room with its [`Member`]s.
#[derive(Debug)]
pub struct Room {
    id: RoomId,

    /// [`RpcConnection`]s of [`Member`]s in this [`Room`].
    pub members: ParticipantService,

    /// [`Peer`]s of [`Member`]s in this [`Room`].
    peers: PeerRepository,
}

impl Room {
    /// Create new instance of [`Room`].
    ///
    /// Returns [`RoomError::BadRoomSpec`] when error while [`Element`]
    /// transformation happens.
    pub fn new(
        room_spec: &RoomSpec,
        reconnect_timeout: Duration,
        turn: Arc<Box<dyn TurnAuthService + Send + Sync>>,
    ) -> Result<Self, RoomError> {
        Ok(Self {
            id: room_spec.id().clone(),
            peers: PeerRepository::from(HashMap::new()),
            members: ParticipantService::new(
                room_spec,
                reconnect_timeout,
                turn,
            )?,
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
            .get_member(&member_id)?
            .servers_list()
            .ok_or_else(|| {
            RoomError::NoTurnCredentials(member_id.clone())
        })?;
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
    ) -> Result<ActFuture<(), RoomError>, RoomError> {
        let from_peer: Peer<WaitLocalSdp> =
            self.peers.take_inner_peer(from_peer_id)?;
        let to_peer_id = from_peer.partner_peer_id();
        let to_peer: Peer<New> = self.peers.take_inner_peer(to_peer_id)?;

        let from_peer = from_peer.set_local_sdp(sdp_offer.clone());
        let to_peer = to_peer.set_remote_sdp(sdp_offer.clone());

        let to_member_id = to_peer.member_id();
        let ice_servers = self
            .members
            .get_member(&to_member_id)?
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
            return Err(RoomError::PeerStateError(PeerStateError::WrongState(
                from_peer_id,
                "Not New",
                format!("{}", from_peer),
            )));
        }

        let to_peer_id = from_peer.partner_peer_id();
        let to_peer = self.peers.get_peer(to_peer_id)?;
        if let PeerStateMachine::New(_) = to_peer {
            return Err(RoomError::PeerStateError(PeerStateError::WrongState(
                to_peer_id,
                "Not New",
                format!("{}", to_peer),
            )));
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
        src: &Rc<WebRtcPublishEndpoint>,
        sink: &Rc<WebRtcPlayEndpoint>,
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
        for (_, publish) in member.srcs() {
            for receiver in publish.sinks() {
                let receiver_owner = receiver.owner();

                if self.members.member_has_connection(&receiver_owner.id())
                    && !receiver.is_connected()
                {
                    if let Some(p) = self.connect_endpoints(&publish, &receiver)
                    {
                        created_peers.push(p)
                    }
                }
            }
        }

        // Create all connected play's receivers peers.
        for (_, play) in member.sinks() {
            let plays_publisher = play.src();
            let plays_publisher_owner = plays_publisher.owner();

            if self
                .members
                .member_has_connection(&plays_publisher_owner.id())
                && !play.is_connected()
            {
                if let Some(p) = self.connect_endpoints(&plays_publisher, &play)
                {
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
                Box::new(res.map_err(|err, _, ctx: &mut Context<Self>| {
                    error!(
                        "Failed handle command, because {}. Room will be \
                         stopped.",
                        err
                    );
                    ctx.notify(CloseRoom {})
                }))
            }
            Err(err) => {
                error!(
                    "Failed handle command, because {}. Room will be stopped.",
                    err
                );
                ctx.notify(CloseRoom {});
                Box::new(wrap_future(future::ok(())))
            }
        };
        ctx.spawn(fut);
    }
}

/// [`Actor`] implementation that provides an ergonomic way
/// to interact with [`Room`].
// TODO: close connections on signal (graceful shutdown)
impl Actor for Room {
    type Context = Context<Self>;
}

impl Into<ElementProto> for &mut Room {
    fn into(self) -> ElementProto {
        let mut element = ElementProto::new();
        let mut room = RoomProto::new();

        let mut pipeline = StdHashMap::new();
        for (id, member) in self.members.members() {
            let local_uri = LocalUri {
                room_id: Some(self.get_id()),
                member_id: Some(id),
                endpoint_id: None,
            };

            pipeline.insert(local_uri.to_string(), member.into());
        }
        room.set_pipeline(pipeline);

        element.set_room(room);

        element
    }
}

/// Serialize this [`Room`] to protobuf object.
#[allow(clippy::module_name_repetitions)]
#[derive(Message)]
#[rtype(result = "Result<ElementProto, RoomError>")]
pub struct SerializeProtobufRoom;

impl Handler<SerializeProtobufRoom> for Room {
    type Result = Result<ElementProto, RoomError>;

    /// Serialize this [`Room`] to protobuf object.
    fn handle(
        &mut self,
        _msg: SerializeProtobufRoom,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        Ok(self.into())
    }
}

/// Serialize [`Member`] from this [`Room`] to protobuf object.
#[derive(Message)]
#[rtype(result = "Result<ElementProto, RoomError>")]
pub struct SerializeProtobufMember(pub MemberId);

impl Handler<SerializeProtobufMember> for Room {
    type Result = Result<ElementProto, RoomError>;

    /// Serialize [`Member`] to protobuf object.
    fn handle(
        &mut self,
        msg: SerializeProtobufMember,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let member = self.members.get_member(&msg.0)?;

        let mut member_element: Room_Element = member.into();
        let member = member_element.take_member();

        let mut element = ElementProto::new();
        element.set_member(member);

        Ok(element)
    }
}

/// Serialize endpoint from this [`Room`] to protobuf object.
#[derive(Message)]
#[rtype(result = "Result<ElementProto, RoomError>")]
pub struct SerializeProtobufEndpoint(pub MemberId, pub String);

impl Handler<SerializeProtobufEndpoint> for Room {
    type Result = Result<ElementProto, RoomError>;

    /// Serialize [`WebRtcPlayEndpoint`] or [`WebRtcPublishEndpoint`] to
    /// protobuf object.
    fn handle(
        &mut self,
        msg: SerializeProtobufEndpoint,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let member = self.members.get_member(&msg.0)?;

        let endpoint_id = WebRtcPublishId(msg.1);
        let mut element = ElementProto::new();

        if let Some(endpoint) = member.get_src_by_id(&endpoint_id) {
            let mut member_element: Member_Element = endpoint.into();
            let endpoint = member_element.take_webrtc_pub();
            element.set_webrtc_pub(endpoint);
        } else {
            let endpoint_id = WebRtcPlayId(endpoint_id.0);

            let endpoint = member.get_sink(&endpoint_id)?;
            let mut member_element: Member_Element = endpoint.into();
            let endpoint = member_element.take_webrtc_play();
            element.set_webrtc_play(endpoint);
        }

        Ok(element)
    }
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
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        info!(
            "Peers {:?} removed for member '{}'.",
            msg.peers_id, msg.member_id
        );
        if let Some(member) = self.members.get_member_by_id(&msg.member_id) {
            member.peers_removed(&msg.peers_id);
        }

        Box::new(
            self.send_peers_removed(msg.member_id, msg.peers_id)
                .map_err(|err, _, ctx: &mut Context<Self>| match err {
                    RoomError::ConnectionNotExists(_) => (),
                    _ => {
                        error!(
                            "Failed PeersEvent command, because {}. Room will \
                             be stopped.",
                            err
                        );
                        ctx.notify(CloseRoom {})
                    }
                }),
        )
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
            Command::MakeSdpOffer { peer_id, sdp_offer } => {
                self.handle_make_sdp_offer(peer_id, sdp_offer)
            }
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
                Box::new(res.map_err(|err, _, ctx: &mut Context<Self>| {
                    error!(
                        "Failed handle command, because {}. Room will be \
                         stopped.",
                        err
                    );
                    ctx.notify(CloseRoom {})
                }))
            }
            Err(err) => {
                error!(
                    "Failed handle command, because {}. Room will be stopped.",
                    err
                );
                ctx.notify(CloseRoom {});
                Box::new(wrap_future(future::ok(())))
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
        info!("RpcConnectionEstablished for member {}", msg.member_id);

        let member_id = msg.member_id;

        let fut = self
            .members
            .connection_established(ctx, member_id.clone(), msg.connection)
            .map_err(|err, _, _| {
                error!("RpcConnectionEstablished error {:?}", err)
            })
            .map(move |member, room, ctx| {
                room.init_member_connections(&member, ctx);
            });
        Box::new(fut)
    }
}

/// Signal of close [`Room`].
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct CloseRoom {}

impl Handler<CloseRoom> for Room {
    type Result = ();

    /// Sends to remote [`Member`] the [`Event`] about [`Peer`] removed.
    /// Closes all active [`RpcConnection`]s.
    fn handle(
        &mut self,
        _msg: CloseRoom,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        info!("Closing Room [id = {:?}]", self.id);
        let drop_fut = self.members.drop_connections(ctx);
        ctx.wait(wrap_future(drop_fut));
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
            .connection_closed(ctx, msg.member_id, &msg.reason);
    }
}

// TODO: maybe better naming
#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct Delete;

impl Handler<Delete> for Room {
    type Result = ();

    fn handle(&mut self, _: Delete, ctx: &mut Self::Context) -> Self::Result {
        for (id, member) in self.members.members() {
            if self.members.member_has_connection(&id) {
                let peer_ids_to_remove: HashSet<PeerId> = member
                    .sinks()
                    .into_iter()
                    .filter_map(|(_, sink)| sink.peer_id())
                    .chain(
                        member
                            .srcs()
                            .into_iter()
                            .flat_map(|(_, src)| src.peer_ids().into_iter()),
                    )
                    .collect();

                let member_id = id.clone();

                self.peers.remove_peers(&member_id, peer_ids_to_remove, ctx);
                self.members.delete_member(&member_id, ctx);
            }
        }
    }
}

/// Signal for delete [`Member`] from this [`Room`]
#[derive(Debug, Message, Clone)]
#[rtype(result = "()")]
pub struct DeleteMember(pub MemberId);

impl Handler<DeleteMember> for Room {
    type Result = ();

    fn handle(
        &mut self,
        msg: DeleteMember,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        debug!("Delete Member [id = {}] in room [id = {}].", msg.0, self.id);
        if let Some(member) = self.members.get_member_by_id(&msg.0) {
            let mut peers = HashSet::new();
            for (_, sink) in member.sinks() {
                if let Some(peer_id) = sink.peer_id() {
                    peers.insert(peer_id);
                }
            }

            for (_, src) in member.srcs() {
                for peer_id in src.peer_ids() {
                    peers.insert(peer_id);
                }
            }

            self.peers.remove_peers(&member.id(), peers, ctx);
        }

        self.members.delete_member(&msg.0, ctx);
    }
}

/// Signal for delete endpoint from this [`Room`]
#[derive(Debug, Message, Clone)]
#[rtype(result = "()")]
pub struct DeleteEndpoint {
    pub member_id: MemberId,
    pub endpoint_id: String,
}

impl Handler<DeleteEndpoint> for Room {
    type Result = ();

    fn handle(
        &mut self,
        msg: DeleteEndpoint,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        if let Some(member) = self.members.get_member_by_id(&msg.member_id) {
            let play_id = WebRtcPlayId(msg.endpoint_id);
            if let Some(endpoint) = member.take_sink(&play_id) {
                if let Some(peer_id) = endpoint.peer_id() {
                    self.peers.remove_peer(&msg.member_id, peer_id, ctx);
                }
            }

            let publish_id = WebRtcPublishId(play_id.0);
            if let Some(endpoint) = member.take_src(&publish_id) {
                let peer_ids = endpoint.peer_ids();
                self.peers.remove_peers(&msg.member_id, peer_ids, ctx);
            }
        } else {
            return;
        }
    }
}

/// Create new [`Member`] in this [`Room`].
#[derive(Message, Debug)]
#[rtype(result = "Result<(), RoomError>")]
pub struct CreateMember(pub MemberId, pub MemberSpec);

impl Handler<CreateMember> for Room {
    type Result = Result<(), RoomError>;

    fn handle(
        &mut self,
        msg: CreateMember,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        self.members.create_member(msg.0, &msg.1)?;
        Ok(())
    }
}

/// Create new `Endpoint` from [`EndpointSpec`].
#[derive(Message, Debug)]
#[rtype(result = "Result<(), RoomError>")]
pub struct CreateEndpoint {
    pub member_id: MemberId,
    pub endpoint_id: String,
    pub spec: EndpointSpec,
}

impl Handler<CreateEndpoint> for Room {
    type Result = Result<(), RoomError>;

    fn handle(
        &mut self,
        msg: CreateEndpoint,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        match msg.spec {
            EndpointSpec::WebRtcPlay(e) => {
                self.members.create_sink_endpoint(
                    &msg.member_id,
                    WebRtcPlayId(msg.endpoint_id),
                    e,
                )?;
            }
            EndpointSpec::WebRtcPublish(e) => {
                self.members.create_src_endpoint(
                    &msg.member_id,
                    WebRtcPublishId(msg.endpoint_id),
                    e,
                )?;
            }
        }

        Ok(())
    }
}

//! Room definitions and implementations. Room is responsible for media
//! connection establishment between concrete [`Member`]s.
//!
//! [`Member`]: crate::signalling::elements::member::Member

use std::collections::{HashMap, HashSet};

use actix::{
    fut::wrap_future, Actor, ActorFuture, AsyncContext, Context, Handler,
    Message, ResponseActFuture, ResponseFuture, WrapFuture as _,
};
use derive_more::Display;
use failure::Fail;
use futures::future;
use medea_client_api_proto::{Command, Event, IceCandidate, PeerId, TrackId};
use medea_control_api_proto::grpc::api::{
    Element as ElementProto, Room as RoomProto,
};

use crate::{
    api::{
        client::rpc_connection::{
            AuthorizationError, Authorize, ClosedReason, CommandMessage,
            RpcConnectionClosed, RpcConnectionEstablished,
        },
        control::{
            callback::{
                callback_repo::CallbackRepository, callback_url::CallbackUrl,
                Callback, CallbackEvent, MemberCallbackEvent,
            },
            endpoints::{
                WebRtcPlayEndpoint as WebRtcPlayEndpointSpec,
                WebRtcPublishEndpoint as WebRtcPublishEndpointSpec,
            },
            refs::{Fid, StatefulFid, ToEndpoint, ToMember},
            room::RoomSpec,
            EndpointId, EndpointSpec, MemberId, MemberSpec, RoomId,
            TryFromElementError, WebRtcPlayId, WebRtcPublishId,
        },
    },
    log::prelude::*,
    media::{
        New, Peer, PeerError, PeerStateMachine, WaitLocalHaveRemote,
        WaitLocalSdp, WaitRemoteSdp,
    },
    shutdown::ShutdownGracefully,
    signalling::{
        elements::{
            endpoints::webrtc::{WebRtcPlayEndpoint, WebRtcPublishEndpoint},
            member::MemberError,
            Member, MembersLoadError,
        },
        participants::{ParticipantService, ParticipantServiceErr},
        peers::PeerRepository,
    },
    AppContext,
};

/// Ergonomic type alias for using [`ActorFuture`] for [`Room`].
pub type ActFuture<I, E> =
    Box<dyn ActorFuture<Actor = Room, Item = I, Error = E>>;

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Fail, Display)]
pub enum RoomError {
    #[display(fmt = "Couldn't find Peer with [id = {}]", _0)]
    PeerNotFound(PeerId),

    MemberError(MemberError),

    #[display(fmt = "Member [id = {}] does not have Turn credentials", _0)]
    NoTurnCredentials(MemberId),

    #[display(fmt = "Couldn't find RpcConnection with Member [id = {}]", _0)]
    ConnectionNotExists(MemberId),

    #[display(fmt = "Unable to send event to Member [id = {}]", _0)]
    UnableToSendEvent(MemberId),

    #[display(fmt = "PeerError: {}", _0)]
    PeerError(PeerError),

    #[display(fmt = "{}", _0)]
    MembersLoadError(MembersLoadError),

    #[display(fmt = "{}", _0)]
    TryFromElementError(TryFromElementError),

    #[display(fmt = "Generic room error: {}", _0)]
    BadRoomSpec(String),

    #[display(fmt = "Turn service error: {}", _0)]
    TurnServiceError(String),

    ParticipantServiceErr(ParticipantServiceErr),

    #[display(fmt = "Client error:{}", _0)]
    ClientError(String),

    #[display(fmt = "Given Fid [fid = {}] to wrong Room [id = {}]", _0, _1)]
    WrongRoomId(StatefulFid, RoomId),

    /// Try to create [`Member`] with ID which already exists.
    #[display(fmt = "Member [id = {}] already exists.", _0)]
    MemberAlreadyExists(Fid<ToMember>),

    /// Try to create [`Endpoint`] with ID which already exists.
    ///
    /// [`Endpoint`]: crate::signalling::elements::endpoints::Endpoint
    #[display(fmt = "Endpoint [id = {}] already exists.", _0)]
    EndpointAlreadyExists(Fid<ToEndpoint>),
}

impl From<PeerError> for RoomError {
    fn from(err: PeerError) -> Self {
        Self::PeerError(err)
    }
}

impl From<TryFromElementError> for RoomError {
    fn from(err: TryFromElementError) -> Self {
        Self::TryFromElementError(err)
    }
}

impl From<MembersLoadError> for RoomError {
    fn from(err: MembersLoadError) -> Self {
        Self::MembersLoadError(err)
    }
}

impl From<ParticipantServiceErr> for RoomError {
    fn from(err: ParticipantServiceErr) -> Self {
        Self::ParticipantServiceErr(err)
    }
}

impl From<MemberError> for RoomError {
    fn from(err: MemberError) -> Self {
        Self::MemberError(err)
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

    callbacks: CallbackRepository,

    /// [`Member`]s and associated [`RpcConnection`]s of this [`Room`], handles
    /// [`RpcConnection`] authorization, establishment, message sending.
    ///
    /// [`RpcConnection`]: crate::api::client::rpc_connection::RpcConnection
    pub members: ParticipantService,

    /// [`Peer`]s of [`Member`]s in this [`Room`].
    peers: PeerRepository,

    /// Current state of this [`Room`].
    state: State,
}

impl Room {
    /// Creates new instance of [`Room`].
    ///
    /// Returns [`RoomError::BadRoomSpec`] when errs while `Element`
    /// transformation happens.
    pub fn new(
        room_spec: &RoomSpec,
        context: &AppContext,
    ) -> Result<Self, RoomError> {
        Ok(Self {
            id: room_spec.id().clone(),
            peers: PeerRepository::from(HashMap::new()),
            members: ParticipantService::new(room_spec, context)?,
            state: State::Started,
            callbacks: context.callbacks.clone(),
        })
    }

    /// Returns [`RoomId`] of this [`Room`].
    pub fn get_id(&self) -> RoomId {
        self.id.clone()
    }

    /// Returns reference to [`RoomId`] of this [`Room`].
    pub fn id(&self) -> &RoomId {
        &self.id
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
        removed_peers_ids: Vec<PeerId>,
    ) -> ActFuture<(), RoomError> {
        Box::new(wrap_future(self.members.send_event_to_member(
            member_id,
            Event::PeersRemoved {
                peer_ids: removed_peers_ids,
            },
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
        mids: HashMap<TrackId, String>,
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
        let from_peer = self.peers.get_peer_by_id(from_peer_id)?;
        if let PeerStateMachine::New(_) = from_peer {
            return Err(PeerError::WrongState(
                from_peer_id,
                "Not New",
                format!("{}", from_peer),
            )
            .into());
        }

        let to_peer_id = from_peer.partner_peer_id();
        let to_peer = self.peers.get_peer_by_id(to_peer_id)?;
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

    /// Creates [`Peer`] for endpoints if [`Peer`] between endpoint's members
    /// doesn't exist.
    ///
    /// Adds `send` track to source member's [`Peer`] and `recv` to
    /// sink member's [`Peer`].
    ///
    /// Returns [`PeerId`]s of newly created [`Peer`] if it has been created.
    ///
    /// # Panics
    ///
    /// Panics if provided endpoints have interconnected [`Peer`]s already.
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

    /// Creates and interconnects all [`Peer`]s between connected [`Member`]
    /// and all available at this moment other [`Member`]s.
    ///
    /// Availability is determined by checking [`RpcConnection`] of all
    /// [`Member`]s from [`WebRtcPlayEndpoint`]s and from receivers of
    /// the connected [`Member`].
    fn init_member_connections(
        &mut self,
        member: &Member,
        ctx: &mut <Self as Actor>::Context,
    ) {
        let mut created_peers: Vec<(PeerId, PeerId)> = Vec::new();

        // Create all connected publish endpoints.
        for publisher in member.srcs().values() {
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
        for receiver in member.sinks().values() {
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

    /// Checks state of interconnected [`Peer`]s and sends [`Event`] about
    /// [`Peer`] created to remote [`Member`].
    fn connect_peers(
        &mut self,
        ctx: &mut Context<Self>,
        first_peer: PeerId,
        second_peer: PeerId,
    ) {
        let fut = match self.send_peer_created(first_peer, second_peer) {
            Ok(res) => {
                Box::new(res.then(|res, room, ctx| -> ActFuture<(), ()> {
                    if res.is_ok() {
                        return Box::new(future::ok(()).into_actor(room));
                    }
                    error!(
                        "Failed connect peers, because {}. Room [id = {}] \
                         will be stopped.",
                        res.unwrap_err(),
                        room.id,
                    );
                    room.close_gracefully(ctx)
                }))
            }
            Err(err) => {
                error!(
                    "Failed connect peers, because {}. Room [id = {}] will be \
                     stopped.",
                    err, self.id,
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
        info!("Closing Room [id = {}]", self.id);
        self.state = State::Stopping;

        Box::new(
            self.members
                .drop_connections(ctx)
                .into_actor(self)
                .map(|_, room: &mut Self, _| {
                    room.state = State::Stopped;
                })
                .map_err(|_, room, _| {
                    error!("Error closing room {:?}", room.id);
                }),
        )
    }

    /// Signals about removing [`Member`]'s [`Peer`]s.
    fn member_peers_removed(
        &mut self,
        peers_id: Vec<PeerId>,
        member_id: MemberId,
        ctx: &mut Context<Self>,
    ) -> ActFuture<(), ()> {
        info!(
            "Peers {:?} removed for member [id = {}].",
            peers_id, member_id
        );
        if let Some(member) = self.members.get_member_by_id(&member_id) {
            member.peers_removed(&peers_id);
        } else {
            error!(
                "Member [id = {}] for which received Event::PeersRemoved not \
                 found. Closing room.",
                member_id
            );

            return Box::new(self.close_gracefully(ctx));
        }

        Box::new(self.send_peers_removed(member_id, peers_id).then(
            |err, room, ctx: &mut Context<Self>| {
                if let Err(e) = err {
                    match e {
                        RoomError::ConnectionNotExists(_)
                        | RoomError::UnableToSendEvent(_) => {
                            Box::new(future::ok(()).into_actor(room))
                        }
                        _ => {
                            error!(
                                "Unexpected failed PeersEvent command, \
                                 because {}. Room will be stopped.",
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

    /// Removes [`Peer`]s and call [`Room::member_peers_removed`] for every
    /// [`Member`].
    ///
    /// This will delete [`Peer`]s from [`PeerRepository`] and send
    /// [`Event::PeersRemoved`] event to [`Member`].
    fn remove_peers(
        &mut self,
        member_id: &MemberId,
        peer_ids_to_remove: HashSet<PeerId>,
        ctx: &mut Context<Self>,
    ) {
        debug!("Remove peers.");
        self.peers
            .remove_peers(&member_id, peer_ids_to_remove)
            .into_iter()
            .for_each(|(member_id, peers_id)| {
                let fut = self.member_peers_removed(peers_id, member_id, ctx);
                ctx.spawn(fut);
            });
    }

    /// Deletes [`Member`] from this [`Room`] by [`MemberId`].
    fn delete_member(&mut self, member_id: &MemberId, ctx: &mut Context<Self>) {
        debug!(
            "Deleting Member [id = {}] in Room [id = {}].",
            member_id, self.id
        );
        if let Some(member) = self.members.get_member_by_id(member_id) {
            let peers: HashSet<PeerId> = member
                .sinks()
                .values()
                .filter_map(WebRtcPlayEndpoint::peer_id)
                .chain(
                    member
                        .srcs()
                        .values()
                        .flat_map(WebRtcPublishEndpoint::peer_ids),
                )
                .collect();

            // Send PeersRemoved to `Member`s which have related to this
            // `Member` `Peer`s.
            self.remove_peers(&member.id(), peers, ctx);

            self.members.delete_member(member_id, ctx);

            debug!(
                "Member [id = {}] deleted from Room [id = {}].",
                member_id, self.id
            );
        }
    }

    /// Deletes endpoint from this [`Room`] by ID.
    fn delete_endpoint(
        &mut self,
        member_id: &MemberId,
        endpoint_id: EndpointId,
        ctx: &mut Context<Self>,
    ) {
        let endpoint_id = if let Some(member) =
            self.members.get_member_by_id(member_id)
        {
            let play_id = endpoint_id.into();
            if let Some(endpoint) = member.take_sink(&play_id) {
                if let Some(peer_id) = endpoint.peer_id() {
                    let removed_peers =
                        self.peers.remove_peer(member_id, peer_id);
                    for (member_id, peers_ids) in removed_peers {
                        let fut = self
                            .member_peers_removed(peers_ids, member_id, ctx);
                        ctx.spawn(fut);
                    }
                }
            }

            let publish_id = String::from(play_id).into();
            if let Some(endpoint) = member.take_src(&publish_id) {
                let peer_ids = endpoint.peer_ids();
                self.remove_peers(member_id, peer_ids, ctx);
            }

            publish_id.into()
        } else {
            endpoint_id
        };

        debug!(
            "Endpoint [id = {}] removed in Member [id = {}] from Room [id = \
             {}].",
            endpoint_id, member_id, self.id
        );
    }

    /// Creates new [`WebRtcPlayEndpoint`] in specified [`Member`].
    ///
    /// This function will check that new [`WebRtcPublishEndpoint`]'s ID is not
    /// present in [`ParticipantService`].
    ///
    /// Returns [`RoomError::EndpointAlreadyExists`] when
    /// [`WebRtcPublishEndpoint`]'s ID already presented in [`Member`].
    pub fn create_src_endpoint(
        &mut self,
        member_id: &MemberId,
        publish_id: WebRtcPublishId,
        spec: WebRtcPublishEndpointSpec,
    ) -> Result<(), RoomError> {
        let member = self.members.get_member(&member_id)?;

        let is_member_have_this_src_id =
            member.get_src_by_id(&publish_id).is_some();

        let play_id = String::from(publish_id).into();
        let is_member_have_this_sink_id =
            member.get_sink_by_id(&play_id).is_some();

        if is_member_have_this_sink_id || is_member_have_this_src_id {
            return Err(RoomError::EndpointAlreadyExists(
                member.get_fid_to_endpoint(play_id.into()),
            ));
        }

        let endpoint = WebRtcPublishEndpoint::new(
            String::from(play_id).into(),
            spec.p2p,
            member.downgrade(),
        );

        debug!(
            "Create WebRtcPublishEndpoint [id = {}] for Member [id = {}] in \
             Room [id = {}]",
            endpoint.id(),
            member_id,
            self.id
        );

        member.insert_src(endpoint);

        Ok(())
    }

    /// Creates new [`WebRtcPlayEndpoint`] in specified [`Member`].
    ///
    /// This function will check that new [`WebRtcPlayEndpoint`]'s ID is not
    /// present in [`ParticipantService`].
    ///
    /// Returns [`RoomError::EndpointAlreadyExists`] when
    /// [`WebRtcPlayEndpoint`]'s ID already presented in [`Member`].
    pub fn create_sink_endpoint(
        &mut self,
        member_id: &MemberId,
        endpoint_id: WebRtcPlayId,
        spec: WebRtcPlayEndpointSpec,
        ctx: &mut Context<Self>,
    ) -> Result<(), RoomError> {
        let member = self.members.get_member(&member_id)?;

        let is_member_have_this_sink_id =
            member.get_sink_by_id(&endpoint_id).is_some();

        let publish_id = String::from(endpoint_id).into();
        let is_member_have_this_src_id =
            member.get_src_by_id(&publish_id).is_some();
        if is_member_have_this_sink_id || is_member_have_this_src_id {
            return Err(RoomError::EndpointAlreadyExists(
                member.get_fid_to_endpoint(publish_id.into()),
            ));
        }

        let partner_member = self.members.get_member(&spec.src.member_id)?;
        let src = partner_member
            .get_src_by_id(&spec.src.endpoint_id)
            .ok_or_else(|| {
                MemberError::EndpointNotFound(
                    partner_member.get_fid_to_endpoint(
                        spec.src.endpoint_id.clone().into(),
                    ),
                )
            })?;

        let sink = WebRtcPlayEndpoint::new(
            String::from(publish_id).into(),
            spec.src,
            src.downgrade(),
            member.downgrade(),
        );

        src.add_sink(sink.downgrade());

        debug!(
            "Created WebRtcPlayEndpoint [id = {}] for Member [id = {}] in \
             Room [id = {}].",
            sink.id(),
            member_id,
            self.id
        );

        member.insert_sink(sink);

        if self.members.member_has_connection(member_id) {
            self.init_member_connections(&member, ctx);
        }

        Ok(())
    }

    /// Creates new [`Member`] in this [`ParticipantService`].
    ///
    /// This function will check that new [`Member`]'s ID is not present in
    /// [`ParticipantService`].
    ///
    /// Returns [`RoomError::MemberAlreadyExists`] when
    /// [`Member`]'s ID already presented in [`ParticipantService`].
    pub fn create_member(
        &mut self,
        id: MemberId,
        spec: &MemberSpec,
    ) -> Result<(), RoomError> {
        if self.members.get_member_by_id(&id).is_some() {
            return Err(RoomError::MemberAlreadyExists(
                self.members.get_fid_to_member(id),
            ));
        }
        let signalling_member = Member::new(
            id.clone(),
            spec.credentials().to_string(),
            self.id.clone(),
        );

        for (id, publish) in spec.publish_endpoints() {
            let signalling_publish = WebRtcPublishEndpoint::new(
                id.clone(),
                publish.p2p.clone(),
                signalling_member.downgrade(),
            );
            signalling_member.insert_src(signalling_publish);
        }

        for (id, play) in spec.play_endpoints() {
            let partner_member =
                self.members.get_member(&play.src.member_id)?;
            let src = partner_member
                .get_src_by_id(&play.src.endpoint_id)
                .ok_or_else(|| {
                    MemberError::EndpointNotFound(
                        partner_member.get_fid_to_endpoint(
                            play.src.endpoint_id.clone().into(),
                        ),
                    )
                })?;

            let sink = WebRtcPlayEndpoint::new(
                id.clone(),
                play.src.clone(),
                src.downgrade(),
                signalling_member.downgrade(),
            );

            signalling_member.insert_sink(sink);
        }

        // This is needed for atomicity.
        for (_, sink) in signalling_member.sinks() {
            let src = sink.src();
            src.add_sink(sink.downgrade());
        }

        self.members.insert_member(id, signalling_member);

        Ok(())
    }

    pub fn send_callback(
        &mut self,
        callback_url: CallbackUrl,
        fid: StatefulFid,
        event: CallbackEvent,
    ) {
        match callback_url {
            CallbackUrl::Grpc(grpc_callback_url) => {
                let callback_service =
                    self.callbacks.get_grpc(&grpc_callback_url);
                callback_service.do_send(Callback::new(fid, event));
            }
        }
    }
}

/// [`Actor`] implementation that provides an ergonomic way
/// to interact with [`Room`].
impl Actor for Room {
    type Context = Context<Self>;

    fn started(&mut self, _: &mut Self::Context) {
        debug!("Room [id = {}] started.", self.id);
    }
}

impl Into<ElementProto> for &mut Room {
    fn into(self) -> ElementProto {
        let mut element = ElementProto::new();
        let mut room = RoomProto::new();

        let pipeline = self
            .members
            .members()
            .into_iter()
            .map(|(id, member)| (id.to_string(), member.into()))
            .collect();

        room.set_pipeline(pipeline);
        room.set_id(self.id().to_string());
        element.set_room(room);

        element
    }
}

// TODO: Tightly coupled with protobuf.
//       We should name this method GetElements, that will return some
//       intermediate DTO, that will be serialized at the caller side.
//       But lets leave it as it is for now.

/// Message for serializing this [`Room`] and [`Room`]'s elements to protobuf
/// spec.
#[derive(Message)]
#[rtype(result = "Result<HashMap<StatefulFid, ElementProto>, RoomError>")]
pub struct SerializeProto(pub Vec<StatefulFid>);

impl Handler<SerializeProto> for Room {
    type Result = Result<HashMap<StatefulFid, ElementProto>, RoomError>;

    fn handle(
        &mut self,
        msg: SerializeProto,
        _: &mut Self::Context,
    ) -> Self::Result {
        let mut serialized: HashMap<StatefulFid, ElementProto> = HashMap::new();
        for fid in msg.0 {
            match &fid {
                StatefulFid::Room(room_fid) => {
                    if room_fid.room_id() == &self.id {
                        let current_room: ElementProto = self.into();
                        serialized.insert(fid, current_room);
                    } else {
                        return Err(RoomError::WrongRoomId(
                            fid,
                            self.id.clone(),
                        ));
                    }
                }
                StatefulFid::Member(member_fid) => {
                    let member =
                        self.members.get_member(member_fid.member_id())?;
                    serialized.insert(fid, member.into());
                }
                StatefulFid::Endpoint(endpoint_fid) => {
                    let member =
                        self.members.get_member(endpoint_fid.member_id())?;
                    let endpoint = member.get_endpoint_by_id(
                        endpoint_fid.endpoint_id().to_string(),
                    )?;
                    serialized.insert(fid, endpoint.into());
                }
            }
        }

        Ok(serialized)
    }
}

impl Handler<Authorize> for Room {
    type Result = Result<(), AuthorizationError>;

    /// Responses with `Ok` if `RpcConnection` is authorized, otherwise `Err`s.
    fn handle(
        &mut self,
        msg: Authorize,
        _: &mut Self::Context,
    ) -> Self::Result {
        self.members
            .get_member_by_id_and_credentials(&msg.member_id, &msg.credentials)
            .map(|_| ())
    }
}

impl Handler<CommandMessage> for Room {
    type Result = ActFuture<(), ()>;

    /// Receives [`Command`] from Web client and passes it to corresponding
    /// handlers. Will emit `CloseRoom` on any error.
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
                // TODO: add E2E test
                if candidate.candidate.is_empty() {
                    warn!("Empty candidate from Peer: {}, ignoring", peer_id);
                    let fut: ActFuture<_, _> = Box::new(actix::fut::ok(()));
                    Ok(fut)
                } else {
                    self.handle_set_ice_candidate(peer_id, candidate)
                }
            }
        };

        match result {
            Ok(res) => {
                Box::new(res.then(|res, room, ctx| -> ActFuture<(), ()> {
                    if res.is_ok() {
                        return Box::new(future::ok(()).into_actor(room));
                    }
                    error!(
                        "Failed handle command, because {}. Room [id = {}] \
                         will be stopped.",
                        res.unwrap_err(),
                        room.id,
                    );
                    room.close_gracefully(ctx)
                }))
            }
            Err(err) => {
                error!(
                    "Failed handle command, because {}. Room [id = {}] will \
                     be stopped.",
                    err, self.id,
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
    /// Creates and interconnects all available [`Member`]'s [`Peer`]s.
    ///
    /// [`RpcConnection`]: crate::api::client::rpc_connection::RpcConnection
    fn handle(
        &mut self,
        msg: RpcConnectionEstablished,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        info!(
            "RpcConnectionEstablished for Member [id = {}].",
            msg.member_id
        );

        let fut = self
            .members
            .connection_established(ctx, msg.member_id, msg.connection)
            .map_err(|err, _, _| {
                error!("RpcConnectionEstablished error {:?}", err)
            })
            .map(|member, room, ctx| {
                room.init_member_connections(&member, ctx);
                let callback_url = member.get_on_join();
                if let Some(callback_url) = member.get_on_join() {
                    room.send_callback(
                        callback_url,
                        member.get_fid().into(),
                        CallbackEvent::Member(MemberCallbackEvent::OnJoin),
                    );
                }
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
            "Room [id = {}] received ShutdownGracefully message so shutting \
             down",
            self.id
        );
        self.close_gracefully(ctx)
    }
}

impl Handler<RpcConnectionClosed> for Room {
    type Result = ();

    /// Passes message to [`ParticipantService`] to cleanup stored connections.
    ///
    /// Removes all related for disconnected [`Member`] [`Peer`]s.
    ///
    /// Sends [`PeersRemoved`] message to [`Member`].
    ///
    /// Deletes all removed [`PeerId`]s from all [`Member`]'s endpoints.
    ///
    /// [`PeersRemoved`]: medea-client-api-proto::Event::PeersRemoved
    fn handle(&mut self, msg: RpcConnectionClosed, ctx: &mut Self::Context) {
        info!(
            "RpcConnectionClosed for member {}, reason {:?}",
            msg.member_id, msg.reason
        );

        self.members
            .connection_closed(msg.member_id.clone(), &msg.reason, ctx);
        // TODO: UNWRAP
        let member = self.members.get_member_by_id(&msg.member_id).unwrap();

        if let ClosedReason::Closed = msg.reason {
            if let Some(callback_url) = member.get_on_leave() {
                self.send_callback(
                    callback_url,
                    member.get_fid().into(),
                    CallbackEvent::Member(MemberCallbackEvent::OnLeave),
                );
            }

            let removed_peers =
                self.peers.remove_peers_related_to_member(&msg.member_id);

            for (peer_member_id, peers_ids) in removed_peers {
                // Here we may have some problems. If two participants
                // disconnect at one moment then sending event
                // to another participant fail,
                // because connection already closed but we don't know about it
                // because message in event loop.
                let fut =
                    self.member_peers_removed(peers_ids, peer_member_id, ctx);
                ctx.spawn(fut);
            }
        }
    }
}

/// Signal for closing this [`Room`].
#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct Close;

impl Handler<Close> for Room {
    type Result = ();

    fn handle(&mut self, _: Close, ctx: &mut Self::Context) -> Self::Result {
        for id in self.members.members().keys() {
            self.delete_member(id, ctx);
        }
        let drop_fut = self.members.drop_connections(ctx);
        ctx.wait(wrap_future(drop_fut));
    }
}

/// Signal for deleting elements from this [`Room`].
#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct Delete(pub Vec<StatefulFid>);

impl Handler<Delete> for Room {
    type Result = ();

    fn handle(&mut self, msg: Delete, ctx: &mut Self::Context) {
        let mut member_ids = Vec::new();
        let mut endpoint_ids = Vec::new();
        for id in msg.0 {
            match id {
                StatefulFid::Member(member_fid) => {
                    member_ids.push(member_fid);
                }
                StatefulFid::Endpoint(endpoint_fid) => {
                    endpoint_ids.push(endpoint_fid);
                }
                _ => warn!("Found Fid<IsRoomId> while deleting __from__ Room."),
            }
        }
        member_ids.into_iter().for_each(|fid| {
            self.delete_member(&fid.member_id(), ctx);
        });
        endpoint_ids.into_iter().for_each(|fid| {
            let (_, member_id, endpoint_id) = fid.take_all();
            self.delete_endpoint(&member_id, endpoint_id, ctx);
        });
    }
}

/// Signal for creating new [`Member`] in this [`Room`].
#[derive(Message, Debug)]
#[rtype(result = "Result<(), RoomError>")]
pub struct CreateMember(pub MemberId, pub MemberSpec);

impl Handler<CreateMember> for Room {
    type Result = Result<(), RoomError>;

    fn handle(
        &mut self,
        msg: CreateMember,
        _: &mut Self::Context,
    ) -> Self::Result {
        self.create_member(msg.0.clone(), &msg.1)?;
        debug!(
            "Member [id = {}] created in Room [id = {}].",
            msg.0, self.id
        );
        Ok(())
    }
}

/// Signal for creating new `Endpoint` from [`EndpointSpec`].
#[derive(Message, Debug)]
#[rtype(result = "Result<(), RoomError>")]
pub struct CreateEndpoint {
    pub member_id: MemberId,
    pub endpoint_id: EndpointId,
    pub spec: EndpointSpec,
}

impl Handler<CreateEndpoint> for Room {
    type Result = Result<(), RoomError>;

    fn handle(
        &mut self,
        msg: CreateEndpoint,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        match msg.spec {
            EndpointSpec::WebRtcPlay(endpoint) => {
                self.create_sink_endpoint(
                    &msg.member_id,
                    msg.endpoint_id.into(),
                    endpoint,
                    ctx,
                )?;
            }
            EndpointSpec::WebRtcPublish(endpoint) => {
                self.create_src_endpoint(
                    &msg.member_id,
                    msg.endpoint_id.into(),
                    endpoint,
                )?;
            }
        }

        Ok(())
    }
}

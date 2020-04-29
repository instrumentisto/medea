//! Room definitions and implementations. Room is responsible for media
//! connection establishment between concrete [`Member`]s.
//!
//! [`Member`]: crate::signalling::elements::member::Member

use std::collections::{HashMap, HashSet};

use actix::{
    Actor, ActorFuture, Addr, Context, ContextFutureSpawner as _, Handler,
    Message, WrapFuture as _,
};
use derive_more::{Display, From};
use failure::Fail;
use futures::future::{FutureExt as _, LocalBoxFuture};
use medea_client_api_proto::{
    Command, CommandHandler, Event, IceCandidate, PeerId, PeerMetrics, TrackId,
    TrackPatch,
};
use medea_control_api_proto::grpc::api as proto;

use crate::{
    api::{
        client::rpc_connection::{
            AuthorizationError, Authorize, ClosedReason, CommandMessage,
            RpcConnection, RpcConnectionClosed, RpcConnectionEstablished,
            RpcConnectionSettings,
        },
        control::{
            callback::{
                clients::CallbackClientFactoryImpl, service::CallbackService,
                OnJoinEvent, OnLeaveEvent, OnLeaveReason,
            },
            endpoints::{
                WebRtcPlayEndpoint as WebRtcPlayEndpointSpec,
                WebRtcPublishEndpoint as WebRtcPublishEndpointSpec,
            },
            refs::{Fid, StatefulFid, ToEndpoint, ToMember},
            room::RoomSpec,
            EndpointId, EndpointSpec, MemberId, MemberSpec, RoomId,
            WebRtcPlayId, WebRtcPublishId,
        },
        RpcServer,
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
    turn::TurnServiceErr,
    utils::ResponseActAnyFuture,
    AppContext,
};

/// Ergonomic type alias for using [`ActorFuture`] for [`Room`].
pub type ActFuture<O> = Box<dyn ActorFuture<Actor = Room, Output = O>>;

#[derive(Debug, Display, Fail, From)]
pub enum RoomError {
    #[display(fmt = "Couldn't find Peer with [id = {}]", _0)]
    PeerNotFound(PeerId),

    MemberError(MemberError),

    #[display(fmt = "Member [id = {}] does not have Turn credentials", _0)]
    #[from(ignore)]
    NoTurnCredentials(MemberId),

    #[display(fmt = "Couldn't find RpcConnection with Member [id = {}]", _0)]
    #[from(ignore)]
    ConnectionNotExists(MemberId),

    #[display(fmt = "Unable to send event to Member [id = {}]", _0)]
    #[from(ignore)]
    UnableToSendEvent(MemberId),

    #[display(fmt = "PeerError: {}", _0)]
    PeerError(PeerError),

    #[display(fmt = "{}", _0)]
    MembersLoadError(MembersLoadError),

    #[display(fmt = "Generic room error: {}", _0)]
    #[from(ignore)]
    BadRoomSpec(String),

    ParticipantServiceErr(ParticipantServiceErr),

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

    /// [`TurnAuthService`] errored to perform an operation.
    ///
    /// [`TurnAuthService`]: crate::turn::service::TurnAuthService
    #[display(fmt = "TurnService errored in Room: {}", _0)]
    TurnServiceErr(TurnServiceErr),
}

/// Error of validating received [`Command`].
#[derive(Debug, Display, Fail, PartialEq)]
pub enum CommandValidationError {
    /// Unable to find expected [`Peer`].
    #[display(fmt = "Couldn't find Peer with [id = {}]", _0)]
    PeerNotFound(PeerId),

    /// Specified [`Peer`] doesn't belong to the [`Member`] which sends
    /// [`Command`].
    #[display(
        fmt = "Peer [id = {}] that doesn't belong to Member [id = {}]",
        _0,
        _1
    )]
    PeerBelongsToAnotherMember(PeerId, MemberId),
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

    /// Service for sending [`CallbackEvent`]s.
    ///
    /// [`CallbackEvent`]: crate::api::control::callbacks::CallbackEvent
    callbacks: CallbackService<CallbackClientFactoryImpl>,

    /// [`Member`]s and associated [`RpcConnection`]s of this [`Room`], handles
    /// [`RpcConnection`] authorization, establishment, message sending.
    ///
    /// [`RpcConnection`]: crate::api::client::rpc_connection::RpcConnection
    pub members: ParticipantService,

    /// [`Peer`]s of [`Member`]s in this [`Room`].
    pub peers: PeerRepository,

    /// Current state of this [`Room`].
    state: State,
}

impl Room {
    /// Creates new instance of [`Room`].
    ///
    /// # Errors
    ///
    /// Errors with [`RoomError::BadRoomSpec`] if [`RoomSpec`] transformation
    /// fails.
    pub fn new(
        room_spec: &RoomSpec,
        context: &AppContext,
    ) -> Result<Self, RoomError> {
        Ok(Self {
            id: room_spec.id().clone(),
            peers: PeerRepository::new(
                room_spec.id().clone(),
                context.turn_service.clone(),
            ),
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
    ) -> Result<ActFuture<Result<(), RoomError>>, RoomError> {
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
        let ice_servers = sender
            .ice_servers_list()
            .ok_or_else(|| RoomError::NoTurnCredentials(member_id.clone()))?;
        let peer_created = Event::PeerCreated {
            peer_id: sender.id(),
            sdp_offer: None,
            tracks: sender.tracks(),
            ice_servers,
            force_relay: sender.is_force_relayed(),
        };
        self.peers.add_peer(sender);
        Ok(Box::new(
            self.members
                .send_event_to_member(member_id, peer_created)
                .into_actor(self),
        ))
    }

    /// Sends [`Event::PeersRemoved`] to [`Member`].
    fn send_peers_removed(
        &mut self,
        member_id: MemberId,
        removed_peers_ids: Vec<PeerId>,
    ) -> ActFuture<Result<(), RoomError>> {
        Box::new(
            self.members
                .send_event_to_member(
                    member_id,
                    Event::PeersRemoved {
                        peer_ids: removed_peers_ids,
                    },
                )
                .into_actor(self),
        )
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
        let mut connect_endpoints_tasks = Vec::new();

        for publisher in member.srcs().values() {
            for receiver in publisher.sinks() {
                let receiver_owner = receiver.owner();

                if receiver.peer_id().is_none()
                    && self.members.member_has_connection(&receiver_owner.id())
                {
                    connect_endpoints_tasks.push(
                        self.peers.connect_endpoints(publisher, &receiver),
                    );
                }
            }
        }

        for receiver in member.sinks().values() {
            let publisher = receiver.src();

            if receiver.peer_id().is_none()
                && self.members.member_has_connection(&publisher.owner().id())
            {
                connect_endpoints_tasks
                    .push(self.peers.connect_endpoints(&publisher, receiver))
            }
        }

        for connect_endpoints_task in connect_endpoints_tasks {
            connect_endpoints_task
                .then(|result, this, _| match result {
                    Ok(Some((peer1, peer2))) => {
                        match this.send_peer_created(peer1, peer2) {
                            Ok(fut) => fut,
                            Err(err) => Box::new(actix::fut::err(err)),
                        }
                    }
                    Err(err) => Box::new(actix::fut::err(err)),
                    _ => Box::new(actix::fut::ok(())),
                })
                .map(|res, _, _| {
                    if let Err(err) = res {
                        error!("Failed connect peers, because {}.", err);
                    }
                })
                .spawn(ctx);
        }
    }

    /// Closes [`Room`] gracefully, by dropping all the connections and moving
    /// into [`State::Stopped`].
    fn close_gracefully(&mut self, ctx: &mut Context<Self>) -> ActFuture<()> {
        info!("Closing Room [id = {}]", self.id);
        self.state = State::Stopping;

        self.members
            .iter_members()
            .filter_map(|(_, member)| {
                member
                    .get_on_leave()
                    .map(move |on_leave| (member, on_leave))
            })
            .filter(|(member, _)| {
                self.members.member_has_connection(&member.id())
            })
            .for_each(|(member, on_leave)| {
                self.callbacks.send_callback(
                    on_leave,
                    member.get_fid().into(),
                    OnLeaveEvent::new(OnLeaveReason::ServerShutdown),
                );
            });

        Box::new(self.members.drop_connections(ctx).into_actor(self).map(
            |_, room: &mut Self, _| {
                room.state = State::Stopped;
            },
        ))
    }

    /// Signals about removing [`Member`]'s [`Peer`]s.
    fn member_peers_removed(
        &mut self,
        peers_id: Vec<PeerId>,
        member_id: MemberId,
        ctx: &mut Context<Self>,
    ) -> ActFuture<()> {
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

            return self.close_gracefully(ctx);
        }

        Box::new(self.send_peers_removed(member_id, peers_id).then(
            |err, this, ctx: &mut Context<Self>| {
                if let Err(e) = err {
                    match e {
                        RoomError::ConnectionNotExists(_)
                        | RoomError::UnableToSendEvent(_) => {
                            Box::new(actix::fut::ready(()))
                        }
                        _ => {
                            error!(
                                "Unexpected failed PeersEvent command, \
                                 because {}. Room will be stopped.",
                                e
                            );
                            this.close_gracefully(ctx)
                        }
                    }
                } else {
                    Box::new(actix::fut::ready(()))
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
        peer_ids_to_remove: &HashSet<PeerId>,
        ctx: &mut Context<Self>,
    ) {
        debug!("Remove peers.");
        self.peers
            .remove_peers(&member_id, &peer_ids_to_remove)
            .into_iter()
            .for_each(|(member_id, peers_id)| {
                self.member_peers_removed(peers_id, member_id, ctx)
                    .map(|_, _, _| ())
                    .spawn(ctx);
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
            self.remove_peers(&member.id(), &peers, ctx);

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
                        self.member_peers_removed(peers_ids, member_id, ctx)
                            .map(|_, _, _| ())
                            .spawn(ctx);
                    }
                }
            }

            let publish_id = String::from(play_id).into();
            if let Some(endpoint) = member.take_src(&publish_id) {
                let peer_ids = endpoint.peer_ids();
                self.remove_peers(member_id, &peer_ids, ctx);
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
    ///
    /// # Errors
    ///
    /// Errors with [`RoomError::ParticipantServiceErr`] if [`Member`] with
    /// provided [`MemberId`] was not found in [`ParticipantService`].
    pub fn create_src_endpoint(
        &mut self,
        member_id: &MemberId,
        publish_id: WebRtcPublishId,
        spec: &WebRtcPublishEndpointSpec,
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
            spec.force_relay,
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
    /// # Errors
    ///
    /// Errors with [`RoomError::EndpointAlreadyExists`] if
    /// [`WebRtcPlayEndpoint`]'s ID already presented in [`Member`].
    ///
    /// Errors with [`RoomError::ParticipantServiceErr`] if [`Member`] with
    /// provided [`MemberId`] doesn't exist.
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
            spec.force_relay,
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

    /// Validates given [`CommandMessage`].
    ///
    /// Two assertions are made:
    /// 1. Specified [`PeerId`] must be known to [`Room`].
    /// 2. Found [`Peer`] must belong to specified [`Member`]
    fn validate_command(
        &self,
        command: &CommandMessage,
    ) -> Result<(), CommandValidationError> {
        use Command as C;
        use CommandValidationError::{
            PeerBelongsToAnotherMember, PeerNotFound,
        };

        let peer_id = match command.command {
            C::MakeSdpOffer { peer_id, .. }
            | C::MakeSdpAnswer { peer_id, .. }
            | C::SetIceCandidate { peer_id, .. }
            | C::AddPeerConnectionMetrics { peer_id, .. }
            | C::UpdateTracks { peer_id, .. } => peer_id,
        };

        let peer = self
            .peers
            .get_peer_by_id(peer_id)
            .map_err(|_| PeerNotFound(peer_id))?;
        if peer.member_id() != command.member_id {
            return Err(PeerBelongsToAnotherMember(peer_id, peer.member_id()));
        }
        Ok(())
    }
}

impl RpcServer for Addr<Room> {
    /// Sends [`RpcConnectionEstablished`] message to [`Room`] actor propagating
    /// errors.
    fn connection_established(
        &self,
        member_id: MemberId,
        connection: Box<dyn RpcConnection>,
    ) -> LocalBoxFuture<'static, Result<(), ()>> {
        self.send(RpcConnectionEstablished {
            member_id,
            connection,
        })
        .map(|res| match res {
            Ok(_) => Ok(()),
            Err(e) => {
                error!(
                    "Failed to send RpcConnectionEstablished cause {:?}",
                    e,
                );
                Err(())
            }
        })
        .boxed_local()
    }

    /// Sends [`RpcConnectionClosed`] message to [`Room`] actor ignoring any
    /// errors.
    fn connection_closed(
        &self,
        member_id: MemberId,
        reason: ClosedReason,
    ) -> LocalBoxFuture<'static, ()> {
        self.send(RpcConnectionClosed { member_id, reason })
            .map(|res| {
                if let Err(e) = res {
                    error!("Failed to send RpcConnectionClosed cause {:?}", e,);
                };
            })
            .boxed_local()
    }

    /// Sends [`CommandMessage`] message to [`Room`] actor ignoring any errors.
    fn send_command(
        &self,
        member_id: MemberId,
        msg: Command,
    ) -> LocalBoxFuture<'static, ()> {
        self.send(CommandMessage::new(member_id, msg))
            .map(|res| {
                if let Err(e) = res {
                    error!("Failed to send CommandMessage cause {:?}", e);
                }
            })
            .boxed_local()
    }
}

impl CommandHandler for Room {
    type Output = Result<ActFuture<Result<(), RoomError>>, RoomError>;

    /// Sends [`Event::PeerCreated`] to provided [`Peer`] partner. Provided
    /// [`Peer`] state must be [`WaitLocalSdp`] and will be changed to
    /// [`WaitRemoteSdp`], partners [`Peer`] state must be [`New`] and will be
    /// changed to [`WaitLocalHaveRemote`].
    fn on_make_sdp_offer(
        &mut self,
        from_peer_id: PeerId,
        sdp_offer: String,
        mids: HashMap<TrackId, String>,
    ) -> Self::Output {
        let mut from_peer: Peer<WaitLocalSdp> =
            self.peers.take_inner_peer(from_peer_id)?;
        from_peer.set_mids(mids)?;

        let to_peer_id = from_peer.partner_peer_id();
        let to_peer: Peer<New> = self.peers.take_inner_peer(to_peer_id)?;

        let from_peer = from_peer.set_local_sdp(sdp_offer.clone());
        let to_peer = to_peer.set_remote_sdp(sdp_offer.clone());

        let to_member_id = to_peer.member_id();
        let ice_servers = to_peer.ice_servers_list().ok_or_else(|| {
            RoomError::NoTurnCredentials(to_member_id.clone())
        })?;

        let event = Event::PeerCreated {
            peer_id: to_peer.id(),
            sdp_offer: Some(sdp_offer),
            tracks: to_peer.tracks(),
            ice_servers,
            force_relay: to_peer.is_force_relayed(),
        };

        self.peers.add_peer(from_peer);
        self.peers.add_peer(to_peer);

        Ok(Box::new(
            self.members
                .send_event_to_member(to_member_id, event)
                .into_actor(self),
        ))
    }

    /// Sends [`Event::SdpAnswerMade`] to provided [`Peer`] partner. Provided
    /// [`Peer`] state must be [`WaitLocalHaveRemote`] and will be changed to
    /// [`Stable`], partners [`Peer`] state must be [`WaitRemoteSdp`] and will
    /// be changed to [`Stable`].
    fn on_make_sdp_answer(
        &mut self,
        from_peer_id: PeerId,
        sdp_answer: String,
    ) -> Self::Output {
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

        Ok(Box::new(
            self.members
                .send_event_to_member(to_member_id, event)
                .into_actor(self),
        ))
    }

    /// Sends [`Event::IceCandidateDiscovered`] to provided [`Peer`] partner.
    /// Both [`Peer`]s may have any state except [`New`].
    fn on_set_ice_candidate(
        &mut self,
        from_peer_id: PeerId,
        candidate: IceCandidate,
    ) -> Self::Output {
        // TODO: add E2E test
        if candidate.candidate.is_empty() {
            warn!("Empty candidate from Peer: {}, ignoring", from_peer_id);
            return Ok(Box::new(actix::fut::ok(())));
        }

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

        Ok(Box::new(
            self.members
                .send_event_to_member(to_member_id, event)
                .into_actor(self),
        ))
    }

    /// Does nothing atm.
    fn on_add_peer_connection_metrics(
        &mut self,
        _: PeerId,
        _: PeerMetrics,
    ) -> Self::Output {
        Ok(Box::new(actix::fut::ok(())))
    }

    /// Sends [`Event::TracksUpdated`] with data from the received
    /// [`Command::UpdateTracks`].
    fn on_update_tracks(
        &mut self,
        peer_id: PeerId,
        tracks_patches: Vec<TrackPatch>,
    ) -> Self::Output {
        if let Ok(p) = self.peers.get_peer_by_id(peer_id) {
            let member_id = p.member_id();
            Ok(Box::new(
                self.members
                    .send_event_to_member(
                        member_id,
                        Event::TracksUpdated {
                            peer_id,
                            tracks_patches,
                        },
                    )
                    .into_actor(self),
            ))
        } else {
            Ok(Box::new(actix::fut::ok(())))
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

impl Into<proto::Room> for &Room {
    fn into(self) -> proto::Room {
        let pipeline = self
            .members
            .members()
            .into_iter()
            .map(|(id, member)| (id.to_string(), member.into()))
            .collect();
        proto::Room {
            id: self.id().to_string(),
            pipeline,
        }
    }
}

impl Into<proto::Element> for &Room {
    fn into(self) -> proto::Element {
        proto::Element {
            el: Some(proto::element::El::Room(self.into())),
        }
    }
}

// TODO: Tightly coupled with protobuf.
//       We should name this method GetElements, that will return some
//       intermediate DTO, that will be serialized at the caller side.
//       But lets leave it as it is for now.

/// Message for serializing this [`Room`] and [`Room`]'s elements to protobuf
/// spec.
#[derive(Message)]
#[rtype(result = "Result<HashMap<StatefulFid, proto::Element>, RoomError>")]
pub struct SerializeProto(pub Vec<StatefulFid>);

impl Handler<SerializeProto> for Room {
    type Result = Result<HashMap<StatefulFid, proto::Element>, RoomError>;

    fn handle(
        &mut self,
        msg: SerializeProto,
        _: &mut Self::Context,
    ) -> Self::Result {
        let mut serialized: HashMap<StatefulFid, proto::Element> =
            HashMap::new();
        for fid in msg.0 {
            match &fid {
                StatefulFid::Room(room_fid) => {
                    if room_fid.room_id() == &self.id {
                        let current_room: proto::Element = (&*self).into();
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
    type Result = Result<RpcConnectionSettings, AuthorizationError>;

    /// Responses with `Ok` if `RpcConnection` is authorized, otherwise `Err`s.
    fn handle(
        &mut self,
        msg: Authorize,
        _: &mut Self::Context,
    ) -> Self::Result {
        self.members
            .get_member_by_id_and_credentials(&msg.member_id, &msg.credentials)
            .map(move |member| RpcConnectionSettings {
                idle_timeout: member.get_idle_timeout(),
                ping_interval: member.get_ping_interval(),
            })
    }
}

impl Handler<CommandMessage> for Room {
    type Result = ResponseActAnyFuture<Self, ()>;

    /// Receives [`Command`] from Web client and passes it to corresponding
    /// handlers. Will emit `CloseRoom` on any error.
    fn handle(
        &mut self,
        msg: CommandMessage,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        let fut = match self.validate_command(&msg) {
            Ok(_) => match msg.command.dispatch_with(self) {
                Ok(res) => {
                    Box::new(res.then(|res, this, ctx| -> ActFuture<()> {
                        if let Err(e) = res {
                            error!(
                                "Failed handle command, because {}. Room [id \
                                 = {}] will be stopped.",
                                e, this.id,
                            );
                            this.close_gracefully(ctx)
                        } else {
                            Box::new(actix::fut::ready(()))
                        }
                    }))
                }
                Err(err) => {
                    error!(
                        "Failed handle command, because {}. Room [id = {}] \
                         will be stopped.",
                        err, self.id,
                    );
                    self.close_gracefully(ctx)
                }
            },
            Err(err) => {
                warn!(
                    "Ignoring Command from Member [{}] that failed validation \
                     cause: {}",
                    msg.member_id, err
                );
                Box::new(actix::fut::ready(()))
            }
        };
        ResponseActAnyFuture(fut)
    }
}

impl Handler<RpcConnectionEstablished> for Room {
    type Result = ActFuture<Result<(), ()>>;

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
            .map(|res, room, ctx| match res {
                Ok(member) => {
                    room.init_member_connections(&member, ctx);
                    if let Some(callback_url) = member.get_on_join() {
                        room.callbacks.send_callback(
                            callback_url,
                            member.get_fid().into(),
                            OnJoinEvent,
                        );
                    };
                    Ok(())
                }
                Err(e) => {
                    error!("RpcConnectionEstablished error {:?}", e);
                    Err(())
                }
            });
        Box::new(fut)
    }
}

impl Handler<ShutdownGracefully> for Room {
    type Result = ResponseActAnyFuture<Self, ()>;

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
        ResponseActAnyFuture(self.close_gracefully(ctx))
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

        if let ClosedReason::Closed { normal } = msg.reason {
            if let Some(member) = self.members.get_member_by_id(&msg.member_id)
            {
                if let Some(on_leave_url) = member.get_on_leave() {
                    let reason = if normal {
                        OnLeaveReason::Disconnected
                    } else {
                        OnLeaveReason::LostConnection
                    };
                    self.callbacks.send_callback(
                        on_leave_url,
                        member.get_fid().into(),
                        OnLeaveEvent::new(reason),
                    );
                }
            } else {
                error!(
                    "Member [id = {}] with ID from RpcConnectionClosed not \
                     found.",
                    msg.member_id,
                );
                self.close_gracefully(ctx).spawn(ctx);
            }

            let removed_peers =
                self.peers.remove_peers_related_to_member(&msg.member_id);

            for (peer_member_id, peers_ids) in removed_peers {
                // Here we may have some problems. If two participants
                // disconnect at one moment then sending event
                // to another participant fail,
                // because connection already closed but we don't know about it
                // because message in event loop.
                self.member_peers_removed(peers_ids, peer_member_id, ctx)
                    .map(|_, _, _| ())
                    .spawn(ctx);
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

    fn handle(&mut self, _: Close, ctx: &mut Self::Context) {
        for id in self.members.members().keys() {
            self.delete_member(id, ctx);
        }
        self.members
            .drop_connections(ctx)
            .into_actor(self)
            .wait(ctx);
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
        self.members.create_member(msg.0.clone(), &msg.1)?;
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
                    &endpoint,
                )?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::{api::control::pipeline::Pipeline, conf::Conf};

    fn empty_room() -> Room {
        let room_spec = RoomSpec {
            id: RoomId::from("test"),
            pipeline: Pipeline::new(HashMap::new()),
        };
        let ctx = AppContext::new(
            Conf::default(),
            crate::turn::new_turn_auth_service_mock(),
        );

        Room::new(&room_spec, &ctx).unwrap()
    }

    #[test]
    fn command_validation_peer_not_found() {
        let mut room = empty_room();

        let member1 = MemberSpec::new(
            Pipeline::new(HashMap::new()),
            String::from("w/e"),
            None,
            None,
            None,
            None,
            None,
        );

        room.members
            .create_member(MemberId(String::from("member1")), &member1)
            .unwrap();

        let no_such_peer = CommandMessage::new(
            MemberId(String::from("member1")),
            Command::SetIceCandidate {
                peer_id: PeerId(1),
                candidate: IceCandidate {
                    candidate: "".to_string(),
                    sdp_m_line_index: None,
                    sdp_mid: None,
                },
            },
        );

        let validation = room.validate_command(&no_such_peer);

        assert_eq!(
            validation,
            Err(CommandValidationError::PeerNotFound(PeerId(1)))
        );
    }

    #[test]
    fn command_validation_peer_does_not_belong_to_member() {
        let mut room = empty_room();

        let member1 = MemberSpec::new(
            Pipeline::new(HashMap::new()),
            String::from("w/e"),
            None,
            None,
            None,
            None,
            None,
        );

        room.members
            .create_member(MemberId(String::from("member1")), &member1)
            .unwrap();

        let no_such_peer = CommandMessage::new(
            MemberId(String::from("member1")),
            Command::SetIceCandidate {
                peer_id: PeerId(1),
                candidate: IceCandidate {
                    candidate: "".to_string(),
                    sdp_m_line_index: None,
                    sdp_mid: None,
                },
            },
        );

        let validation = room.validate_command(&no_such_peer);

        assert_eq!(
            validation,
            Err(CommandValidationError::PeerNotFound(PeerId(1)))
        );
    }
}

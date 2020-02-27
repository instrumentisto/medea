//! Room definitions and implementations. Room is responsible for media
//! connection establishment between concrete [`Member`]s.
//!
//! [`Member`]: crate::signalling::elements::member::Member

use std::collections::{HashMap, HashSet};

use actix::{
    Actor, ActorFuture, Addr, AsyncContext, Context, ContextFutureSpawner as _,
    Handler, Message, WrapFuture as _, WrapFuture,
};
use derive_more::Display;
use failure::Fail;
use futures::future::{self, FutureExt as _, LocalBoxFuture};
use medea_client_api_proto::{
    Command, CommandHandler, Event, IceCandidate, PeerId, PeerMetrics, TrackId,
};
use medea_control_api_proto::grpc::api as proto;

use crate::{
    api::{
        client::rpc_connection::{
            AuthorizationError, Authorize, ClosedReason, CommandMessage,
            RpcConnection, RpcConnectionClosed, RpcConnectionEstablished,
        },
        control::{
            callback::{
                clients::CallbackClientFactoryImpl, service::CallbackService,
                OnJoinEvent, OnLeaveEvent, OnLeaveReason, OnStartEvent,
                OnStopEvent,
            },
            endpoints::{
                webrtc_play_endpoint::Validated,
                WebRtcPlayEndpoint as WebRtcPlayEndpointSpec,
                WebRtcPublishEndpoint as WebRtcPublishEndpointSpec,
            },
            refs::{Fid, StatefulFid, ToEndpoint, ToMember},
            room::RoomSpec,
            EndpointId, EndpointSpec, MemberId, MemberSpec, RoomId,
            TryFromElementError, WebRtcPlayId, WebRtcPublishId,
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
    turn::coturn_stats::{CoturnStats, EventType, Subscribe},
    utils::ResponseActAnyFuture,
    AppContext,
};
use futures::TryFutureExt;

/// Ergonomic type alias for using [`ActorFuture`] for [`Room`].
pub type ActFuture<O> = Box<dyn ActorFuture<Actor = Room, Output = O>>;

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
    peers: PeerRepository,

    /// Current state of this [`Room`].
    state: State,

    coturn_stats: Addr<CoturnStats>,
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
            coturn_stats: context.coturn_stats.clone(),
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
        let sender_peer_id = sender.id();
        self.peers.add_peer(sender);
        let fut = self.peers.get_ice_user(sender_peer_id);

        Ok(Box::new(fut.into_actor(self).then(
            move |ice_user, this, ctx| {
                let sender = this.peers.get_peer_by_id(sender_peer_id).unwrap();
                let peer_created = Event::PeerCreated {
                    peer_id: sender.id(),
                    sdp_offer: None,
                    tracks: sender.tracks(),
                    ice_servers: ice_user.servers_list(),
                    force_relay: sender.is_force_relayed(),
                };

                this.members
                    .send_event_to_member(member_id, peer_created)
                    .into_actor(this)
            },
        )))
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
        member
            .srcs()
            .into_iter()
            .flat_map(|(_, publisher)| {
                publisher
                    .sinks()
                    .into_iter()
                    .map(move |receiver| (publisher.clone(), receiver))
            })
            .filter({
                let members = &self.members;
                move |(_, receiver)| {
                    receiver.peer_id().is_none()
                        && members.member_has_connection(&receiver.owner().id())
                }
            })
            .chain(
                member
                    .sinks()
                    .into_iter()
                    .map(|(_, receiver)| (receiver.src(), receiver))
                    .filter({
                        let members = &self.members;
                        move |(publisher, receiver)| {
                            receiver.peer_id().is_none()
                                && members.member_has_connection(
                                    &publisher.owner().id(),
                                )
                        }
                    }),
            )
            .filter_map({
                let peers = &mut self.peers;
                move |(publisher, receiver)| {
                    peers.connect_endpoints(&publisher, &receiver)
                }
            })
            .collect::<Vec<_>>()
            .into_iter()
            .for_each(|(first_peer_id, second_peer_id)| {
                self.connect_peers(ctx, first_peer_id, second_peer_id);
            });
    }

    /// Checks state of interconnected [`Peer`]s and sends [`Event`] about
    /// [`Peer`] created to remote [`Member`].
    fn connect_peers(
        &mut self,
        ctx: &mut Context<Self>,
        first_peer: PeerId,
        second_peer: PeerId,
    ) {
        self.coturn_stats.do_send(Subscribe {
            peer_id: first_peer,
            partner_peer_id: second_peer,
            room_id: self.id.clone(),
            addr: ctx.address(),
            events_type: HashSet::new(),
        });
        self.coturn_stats.do_send(Subscribe {
            peer_id: second_peer,
            partner_peer_id: first_peer,
            room_id: self.id.clone(),
            addr: ctx.address(),
            events_type: HashSet::new(),
        });
        match self.send_peer_created(first_peer, second_peer) {
            Ok(res) => Box::new(res.then(|res, this, ctx| -> ActFuture<()> {
                if res.is_ok() {
                    return Box::new(future::ready(()).into_actor(this));
                }
                error!(
                    "Failed connect peers, because {}. Room [id = {}] will be \
                     stopped.",
                    res.unwrap_err(),
                    this.id,
                );
                this.close_gracefully(ctx)
            })),
            Err(err) => {
                error!(
                    "Failed connect peers, because {}. Room [id = {}] will be \
                     stopped.",
                    err, self.id,
                );
                self.close_gracefully(ctx)
            }
        }
        .spawn(ctx);
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
                            Box::new(future::ready(()).into_actor(this))
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
                    Box::new(future::ready(()).into_actor(this))
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
                        self.member_peers_removed(peers_ids, member_id, ctx)
                            .map(|_, _, _| ())
                            .spawn(ctx);
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
    ///
    /// # Errors
    ///
    /// Errors with [`RoomError::ParticipantServiceErr`] if [`Member`] with
    /// provided [`MemberId`] was not found in [`ParticipantService`].
    pub fn create_src_endpoint(
        &mut self,
        member_id: &MemberId,
        publish_id: WebRtcPublishId,
        spec: &WebRtcPublishEndpointSpec<Validated>,
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
            spec.on_start.clone(),
            spec.on_stop.clone(),
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
        spec: WebRtcPlayEndpointSpec<Validated>,
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
            spec.on_start,
            spec.on_stop,
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
    /// # Errors
    ///
    /// Errors with [`RoomError::MemberAlreadyExists`] if [`Member`] with
    /// provided [`MemberId`] already exists in [`ParticipantService`].
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

        signalling_member.set_callback_urls(spec);

        for (id, publish) in spec.publish_endpoints() {
            let signalling_publish = WebRtcPublishEndpoint::new(
                id.clone(),
                publish.p2p,
                signalling_member.downgrade(),
                publish.force_relay,
                publish.on_start.clone(),
                publish.on_stop.clone(),
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
                play.force_relay,
                play.on_start.clone(),
                play.on_stop.clone(),
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
    fn send_command(&self, msg: Command) -> LocalBoxFuture<'static, ()> {
        self.send(CommandMessage::from(msg))
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
        let to_peer_id = to_peer.id();

        self.peers.add_peer(from_peer);
        self.peers.add_peer(to_peer);

        let fut = self.peers.get_ice_user(to_peer_id);

        Ok(Box::new(fut.into_actor(self).then(
            move |ice_user, this, ctx| {
                let to_peer = this.peers.get_peer_by_id(to_peer_id).unwrap();
                let event = Event::PeerCreated {
                    peer_id: to_peer.id(),
                    sdp_offer: Some(sdp_offer),
                    tracks: to_peer.tracks(),
                    ice_servers: ice_user.servers_list(),
                    force_relay: to_peer.is_force_relayed(),
                };

                this.members
                    .send_event_to_member(to_member_id, event)
                    .into_actor(this)
            },
        )))
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
            return Ok(Box::new(future::ok(()).into_actor(self)));
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
        _peer_id: PeerId,
        _candidate: PeerMetrics,
    ) -> Self::Output {
        Ok(Box::new(future::ok(()).into_actor(self)))
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
    type Result = ResponseActAnyFuture<Self, ()>;

    /// Receives [`Command`] from Web client and passes it to corresponding
    /// handlers. Will emit `CloseRoom` on any error.
    fn handle(
        &mut self,
        msg: CommandMessage,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        let fut = match Command::from(msg).dispatch_with(self) {
            Ok(res) => Box::new(res.then(|res, this, ctx| -> ActFuture<()> {
                if let Err(e) = res {
                    error!(
                        "Failed handle command, because {}. Room [id = {}] \
                         will be stopped.",
                        e, this.id,
                    );
                    this.close_gracefully(ctx)
                } else {
                    Box::new(future::ready(()).into_actor(this))
                }
            })),
            Err(err) => {
                error!(
                    "Failed handle command, because {}. Room [id = {}] will \
                     be stopped.",
                    err, self.id,
                );
                self.close_gracefully(ctx)
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
                    &endpoint,
                )?;
            }
        }

        Ok(())
    }
}

#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct OnStartOnStopCallback {
    pub event: EventType,
    pub peer_id: PeerId,
}

impl Handler<OnStartOnStopCallback> for Room {
    type Result = ();

    fn handle(
        &mut self,
        msg: OnStartOnStopCallback,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        let endpoint = self
            .peers
            .get_endpoint_path_by_peer_id(msg.peer_id)
            .and_then(|endpoint| endpoint.upgrade());

        use super::elements::endpoints::Endpoint;
        if let Some(endpoint) = endpoint {
            match endpoint {
                Endpoint::WebRtcPlayEndpoint(play_endpoint) => {
                    let fid = play_endpoint
                        .owner()
                        .get_fid_to_endpoint(play_endpoint.id().into());
                    match msg.event {
                        EventType::OnStart => {
                            if let Some(callback_url) = play_endpoint.on_start()
                            {
                                self.callbacks.send_callback(
                                    callback_url,
                                    fid.into(),
                                    OnStartEvent,
                                );
                            }
                        }
                        EventType::OnStop => {
                            if let Some(callback_url) = play_endpoint.on_stop()
                            {
                                self.callbacks.send_callback(
                                    callback_url,
                                    fid.into(),
                                    OnStopEvent,
                                );
                            }
                        }
                    }
                }
                Endpoint::WebRtcPublishEndpoint(publish_endpoint) => {
                    // TODO: I'm redudant in some if cases!!!
                    let fid = publish_endpoint
                        .owner()
                        .get_fid_to_endpoint(publish_endpoint.id().into());
                    match msg.event {
                        EventType::OnStart => {
                            publish_endpoint
                                .change_peer_status(msg.peer_id, true);
                            if let Some(on_start) = publish_endpoint.on_start()
                            {
                                if publish_endpoint.publishing_peers_count()
                                    == 1
                                {
                                    self.callbacks.send_callback(
                                        on_start,
                                        fid.into(),
                                        OnStartEvent,
                                    );
                                }
                            }
                        }
                        EventType::OnStop => {
                            publish_endpoint
                                .change_peer_status(msg.peer_id, true);
                            if let Some(on_stop) = publish_endpoint.on_stop() {
                                if !publish_endpoint.is_endpoint_publishing() {
                                    self.callbacks.send_callback(
                                        on_stop,
                                        fid.into(),
                                        OnStopEvent,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        debug!("LOOK AT ME: {:?}", msg);
    }
}

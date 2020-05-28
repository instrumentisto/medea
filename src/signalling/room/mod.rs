//! Room definitions and implementations. Room is responsible for media
//! connection establishment between concrete [`Member`]s.

mod command_handler;
mod dynamic_api;
mod peer_events_handler;
mod rpc_server;

use std::sync::Arc;

use actix::{
    Actor, ActorFuture, AsyncContext, Context, ContextFutureSpawner as _,
    Handler, MailboxError, WrapFuture as _,
};
use derive_more::{Display, From};
use failure::Fail;
use medea_client_api_proto::{Event, PeerId};

use crate::{
    api::control::{
        callback::{
            clients::CallbackClientFactoryImpl, service::CallbackService,
            OnLeaveEvent, OnLeaveReason,
        },
        refs::{Fid, StatefulFid, ToEndpoint, ToMember},
        room::RoomSpec,
        MemberId, RoomId,
    },
    log::prelude::*,
    media::{New, Peer, PeerError},
    shutdown::ShutdownGracefully,
    signalling::{
        elements::{member::MemberError, Member, MembersLoadError},
        participants::{ParticipantService, ParticipantServiceErr},
        peers::{PeerTrafficWatcher, PeersService},
    },
    turn::TurnServiceErr,
    utils::ResponseActAnyFuture,
    AppContext,
};

use crate::api::control::callback::CallbackRequest;
pub use dynamic_api::{
    Close, CreateEndpoint, CreateMember, Delete, SerializeProto,
};
use std::time::Duration;

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

    /// [`MailboxError`] return on sending message to the
    /// [`PeerTrafficWatcher`] service. service.
    #[display(
        fmt = "Mailbox error while sending message to the PeerTrafficWatcher \
               service: {}",
        _0
    )]
    #[from(ignore)]
    PeerTrafficWatcherMailbox(MailboxError),
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
    /// [`CallbackEvent`]: crate::api::control::callback::CallbackEvent
    callbacks: CallbackService<CallbackClientFactoryImpl>,

    /// [`Member`]s and associated [`RpcConnection`]s of this [`Room`], handles
    /// [`RpcConnection`] authorization, establishment, message sending.
    ///
    /// [`RpcConnection`]: crate::api::client::rpc_connection::RpcConnection
    pub members: ParticipantService,

    /// [`Peer`]s of [`Member`]s in this [`Room`].
    pub peers: PeersService,

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
        peers_traffic_watcher: Arc<dyn PeerTrafficWatcher>,
    ) -> Result<Self, RoomError> {
        Ok(Self {
            id: room_spec.id().clone(),
            peers: PeersService::new(
                room_spec.id().clone(),
                context.turn_service.clone(),
                peers_traffic_watcher,
                &context.config.media,
            ),
            members: ParticipantService::new(room_spec, context)?,
            state: State::Started,
            callbacks: context.callbacks.clone(),
        })
    }

    /// Returns [`RoomId`] of this [`Room`].
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

        for (_, publisher) in member.srcs().drain() {
            for receiver in publisher.sinks() {
                let receiver_owner = receiver.owner();

                if receiver.peer_id().is_none()
                    && self.members.member_has_connection(&receiver_owner.id())
                {
                    connect_endpoints_tasks.push(
                        self.peers
                            .connect_endpoints(publisher.clone(), receiver),
                    );
                }
            }
        }

        for (_, receiver) in member.sinks().drain() {
            let publisher = receiver.src();

            if receiver.peer_id().is_none()
                && self.members.member_has_connection(&publisher.owner().id())
            {
                connect_endpoints_tasks
                    .push(self.peers.connect_endpoints(publisher, receiver))
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
                    CallbackRequest::new_at_now(
                        member.get_fid(),
                        OnLeaveEvent::new(OnLeaveReason::ServerShutdown),
                    ),
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
            "Peers {:?} removed for Member [id = {}].",
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
}

/// [`Actor`] implementation that provides an ergonomic way
/// to interact with [`Room`].
impl Actor for Room {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        debug!("Room [id = {}] started.", self.id);
        ctx.run_interval(Duration::from_secs(1), |this, _| {
            this.peers.check_peers();
        });
        ctx.add_stream(self.peers.subscribe_to_metrics_events());
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

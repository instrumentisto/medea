//! Room definitions and implementations. Room is responsible for media
//! connection establishment between concrete [`Member`]s.

mod command_handler;
mod dynamic_api;
mod peer_events_handler;
mod rpc_server;

use std::{pin::Pin, rc::Rc, sync::Arc, time::Duration};

use actix::{
    Actor, ActorFuture, Addr, AsyncContext as _, AtomicResponse, Context,
    Handler, MailboxError, WrapFuture as _,
};
use derive_more::{Display, From};
use failure::Fail;
use futures::future;
use medea_client_api_proto::{
    state, CloseReason, Event, MemberId, NegotiationRole, PeerId, RoomId,
};

use crate::{
    api::control::{
        callback::{
            CallbackClientError, CallbackClientFactoryImpl, CallbackService,
            OnLeaveEvent, OnLeaveReason,
        },
        refs::{Fid, StatefulFid, ToEndpoint, ToMember},
        room::RoomSpec,
    },
    log::prelude::*,
    media::{peer::PeerUpdatesSubscriber, Peer, PeerError, Stable},
    shutdown::ShutdownGracefully,
    signalling::{
        elements::{member::MemberError, Member, MembersLoadError},
        participants::{ParticipantService, ParticipantServiceErr},
        peers::{PeerTrafficWatcher, PeersService},
    },
    turn::TurnServiceErr,
    utils::actix_try_join_all,
    AppContext,
};

pub use dynamic_api::{
    Close, CreateEndpoint, CreateMember, Delete, SerializeProto,
};

/// Ergonomic type alias for using [`ActorFuture`] for [`Room`].
pub type ActFuture<O = ()> =
    Pin<Box<dyn ActorFuture<Actor = Room, Output = O>>>;

#[derive(Debug, Display, Fail, From)]
pub enum RoomError {
    #[display(fmt = "Couldn't find Peer with [id = {}]", _0)]
    PeerNotFound(PeerId),

    MemberError(MemberError),

    #[display(fmt = "Could not find Member with provided Id or Credential")]
    AuthorizationError,

    #[display(fmt = "Member [id = {}] does not have Turn credentials", _0)]
    #[from(ignore)]
    NoTurnCredentials(MemberId),

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

    /// [`MailboxError`] returned on sending message to [`PeerTrafficWatcher`]
    /// service.
    #[display(
        fmt = "Mailbox error while sending message to PeerTrafficWatcher \
               service: {}",
        _0
    )]
    #[from(ignore)]
    PeerTrafficWatcherMailbox(MailboxError),

    /// Failed to send callback via [`CallbackService`]
    #[display(fmt = "CallbackService errored in Room: {}", _0)]
    CallbackClientError(CallbackClientError),
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
    members: ParticipantService,

    /// [`Peer`]s of [`Member`]s in this [`Room`].
    peers: Rc<PeersService>,
}

impl Room {
    /// Creates and starts [`Room`] [`Actor`] on current thread.
    ///
    /// Returns [`Addr`] to the newly created [`Room`] [`Actor`].
    ///
    /// # Errors
    ///
    /// Errors with [`RoomError::BadRoomSpec`] if [`RoomSpec`] transformation
    /// fails.
    pub fn start(
        room_spec: &RoomSpec,
        context: &AppContext,
        peers_traffic_watcher: Arc<dyn PeerTrafficWatcher>,
    ) -> Result<Addr<Self>, RoomError> {
        // 16 is the default actix address channel capacity.
        let (_, rx) = actix::dev::channel::channel(16);

        let ctx = Context::with_receiver(rx);
        let this = Self {
            id: room_spec.id().clone(),
            peers: PeersService::new(
                room_spec.id().clone(),
                context.turn_service.clone(),
                peers_traffic_watcher,
                &context.config.media,
                Rc::new(ctx.address().downgrade())
                    as Rc<dyn PeerUpdatesSubscriber>,
            ),
            members: ParticipantService::new(room_spec, context)?,
            callbacks: context.callbacks.clone(),
        };

        Ok(ctx.run(this))
    }

    /// Returns [`state::Room`] for the provided [`MemberId`].
    #[inline]
    #[must_use]
    pub fn get_state(&self, member_id: &MemberId) -> state::Room {
        state::Room {
            peers: self.peers.get_peers_states(member_id),
        }
    }

    /// Returns [`RoomId`] of this [`Room`].
    #[inline]
    #[must_use]
    pub fn id(&self) -> &RoomId {
        &self.id
    }

    /// Sends [`Event::PeersRemoved`] to [`Member`].
    #[inline]
    fn send_peers_removed(
        &self,
        member_id: &MemberId,
        removed_peers_ids: Vec<PeerId>,
    ) {
        self.members.send_event_to_member(
            member_id,
            Event::PeersRemoved {
                peer_ids: removed_peers_ids,
            },
        )
    }

    /// Connects interconnected `Endpoint`s between provided [`Member`]s.
    fn connect_members(
        &mut self,
        member1: &Member,
        member2: &Member,
    ) -> ActFuture<Result<(), RoomError>> {
        let member2_id = member2.id();
        let mut connect_endpoints_tasks = Vec::new();

        for src in member1.srcs().values() {
            for sink in src.sinks() {
                if sink.owner().id() == member2_id {
                    connect_endpoints_tasks.push(
                        self.peers.clone().connect_endpoints(src.clone(), sink),
                    );
                }
            }
        }

        for sink in member1.sinks().values() {
            let src = sink.src();
            if src.owner().id() == member2_id {
                connect_endpoints_tasks.push(
                    self.peers.clone().connect_endpoints(src, sink.clone()),
                )
            }
        }

        Box::pin(
            future::try_join_all(connect_endpoints_tasks)
                .into_actor(self)
                .map(move |result, room: &mut Room, _| {
                    for (src_peer_id, _) in
                        result?.into_iter().filter_map(|r| r)
                    {
                        room.peers.commit_scheduled_changes(src_peer_id)?;
                    }

                    Ok(())
                }),
        )
    }

    /// Creates and interconnects all [`Peer`]s between connected [`Member`]
    /// and all available at this moment other [`Member`]s. Expects that
    /// provided [`Member`] have active [`RpcConnection`].
    ///
    /// Availability is determined by checking [`RpcConnection`] of all
    /// [`Member`]s from `WebRtcPlayEndpoint`s and from receivers of
    /// the connected [`Member`].
    ///
    /// Will start (re)negotiation with `MediaTrack`s adding if some not
    /// interconnected `Endpoint`s will be found and if [`Peer`]s pair is
    /// already exists.
    ///
    /// [`RpcConnection`]: crate::api::client::rpc_connection::RpcConnection
    fn init_member_connections(
        &mut self,
        member: &Member,
    ) -> ActFuture<Result<(), RoomError>> {
        let connect_members_tasks =
            member.partners().into_iter().filter_map(|partner| {
                if self.members.member_has_connection(&partner.id()) {
                    Some(self.connect_members(&partner, member))
                } else {
                    None
                }
            });

        Box::pin(
            actix_try_join_all(connect_members_tasks)
                .map(|result, _, _| result.map(drop)),
        )
    }

    /// Signals about removing [`Member`]'s [`Peer`]s.
    fn member_peers_removed(
        &mut self,
        peers_id: Vec<PeerId>,
        member_id: &MemberId,
    ) {
        info!(
            "Peers {:?} removed for member [id = {}].",
            peers_id, member_id
        );
        if let Ok(member) = self.members.get_member_by_id(&member_id) {
            member.peers_removed(&peers_id);
            self.send_peers_removed(member_id, peers_id);
        }
    }

    /// Sends [`Event::PeerUpdated`] with latest [`Peer`] changes to specified
    /// [`Member`]. Starts renegotiation, marking provided [`Peer`] as
    /// [`NegotiationRole::Offerer`].
    ///
    /// # Errors
    ///
    /// Errors if [`Peer`] lookup fails, or it is not in [`Stable`] state.
    fn send_tracks_applied(&self, peer_id: PeerId) -> Result<(), RoomError> {
        let peer: Peer<Stable> = self.peers.take_inner_peer(peer_id)?;
        let partner_peer: Peer<Stable> =
            self.peers.take_inner_peer(peer.partner_peer_id())?;

        let peer = peer.start_as_offerer();
        let partner_peer = partner_peer.start_as_answerer();

        let updates = peer.get_updates();
        let member_id = peer.member_id();

        self.members.send_event_to_member(
            member_id,
            Event::PeerUpdated {
                updates,
                negotiation_role: Some(NegotiationRole::Offerer),
                peer_id,
            },
        );

        self.peers.add_peer(peer);
        self.peers.add_peer(partner_peer);

        Ok(())
    }

    /// Closes [`Member`]s [`RpcConnection`] if `ws_close_reason` is provided,
    /// removes [`Member`]s [`Peer`], notifying connected [`Members`] and emits
    /// [`OnLeaveEvent`] [`CallbackEvent`] if `on_leave_reason` is provided and
    /// [`Member`] is configured to emit [`OnLeaveEvent`].
    ///
    /// [`CallbackEvent`]: crate::api::control::callback::CallbackEvent
    fn disconnect_member(
        &mut self,
        member_id: &MemberId,
        ws_close_reason: CloseReason,
        on_leave_reason: Option<OnLeaveReason>,
        ctx: &mut Context<Room>,
    ) {
        let removed_peers =
            self.peers.remove_peers_related_to_member(&member_id);
        for (peer_member_id, peers_ids) in removed_peers {
            self.member_peers_removed(peers_ids, &peer_member_id);
        }

        self.members
            .close_member_connection(&member_id, ws_close_reason, ctx);

        if let Ok(member) = self.members.get_member_by_id(member_id) {
            if let (Some(url), Some(reason)) =
                (member.get_on_leave(), on_leave_reason)
            {
                self.callbacks.do_send(
                    url,
                    member.get_fid().into(),
                    OnLeaveEvent::new(reason),
                );
            }
        }
    }
}

/// [`Actor`] implementation that provides an ergonomic way
/// to interact with [`Room`].
impl Actor for Room {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        debug!("Room [id = {}] started.", self.id);
        ctx.run_interval(Duration::from_secs(5), |this, _| {
            this.peers.check_peers();
        });
        ctx.add_stream(self.peers.subscribe_to_metrics_events());
    }
}

impl Handler<ShutdownGracefully> for Room {
    type Result = AtomicResponse<Self, ()>;

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
                self.callbacks.do_send(
                    on_leave,
                    member.get_fid().into(),
                    OnLeaveEvent::new(OnLeaveReason::ServerShutdown),
                );
            });

        AtomicResponse::new(Box::pin(
            self.members.drop_connections(ctx).into_actor(self),
        ))
    }
}

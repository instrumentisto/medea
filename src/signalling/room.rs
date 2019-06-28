//! Room definitions and implementations. Room is responsible for media
//! connection establishment between concrete [`Member`]s.

use std::time::Duration;

use actix::{
    fut::wrap_future, Actor, ActorFuture, AsyncContext, Context, Handler,
    Message,
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
        control::{RoomId, RoomSpec, TryFromElementError},
    },
    log::prelude::*,
    media::{
        New, Peer, PeerId, PeerStateError, PeerStateMachine,
        WaitLocalHaveRemote, WaitLocalSdp, WaitRemoteSdp,
    },
    signalling::{
        control::member::MemberId, peers::PeerRepository, pipeline::Pipeline,
    },
    turn::TurnAuthService,
};

/// Ergonomic type alias for using [`ActorFuture`] for [`Room`].
pub type ActFuture<I, E> =
    Box<dyn ActorFuture<Actor = Room, Item = I, Error = E>>;

/// Macro for unwrapping Option.
///
/// If [`Option::None`] then `error!` with provided message will be
/// called, [`CloseRoom`] emitted to [`Room`] context and function
/// will be returned with provided return expression.
///
/// You can use `format!` syntax in this macro to provide some debug info.
///
/// ## Usage
/// ```ignore
/// option_unwrap!(
///     foo.some_weak_pointer().upgrade(), // Some Option type
///     ctx, // Context of Room
///     (), // This will be returned from function in None case
///     "Empty Weak pointer for bar with ID {}", // Error message
///     foo.id(), // format! syntax
/// );
/// ```
macro_rules! option_unwrap {
    ($e:expr, $ctx:expr, $ret:expr, $msg:expr, $( $x:expr ),* $(,)?) => {
        if let Some(e) = $e {
            e
        } else {
            error!(
                "[ROOM]: {} Room will be closed.",
                format!($msg, $( $x, )*)
            );
            $ctx.notify(CloseRoom {});
            return $ret;
        }
    };

    ($e:expr, $ctx:expr, $ret:expr, $msg:expr $(,)?) => {
        if let Some(e) = $e {
            e
        } else {
            error!("[ROOM]: {} Room will be closed.", $msg);
            $ctx.notify(CloseRoom {});
            return $ret;
        }
    };
}

/// Macro for unwrapping Option that work similar as [`option_unwrap!`], but
/// always return `()` when None case happened. This will close Room when
/// `Option::None`.
///
/// Read more info in [`option_unwrap!`] docs.
///
/// ## Usage
/// ```ignore
/// unit_option_unwrap!(
///     foo.some_weak_pointer().upgrade(), // Some Option type
///     ctx, // Context of Room
///     "Empty Weak pointer for bar with ID {}", // Error message
///     foo.id(), // format! syntax
/// );
/// ```
macro_rules! unit_option_unwrap {
    ($e:expr, $ctx:expr, $msg:tt, $( $x:expr ),* $(,)?) => {
        option_unwrap!($e, $ctx, (), $msg, $( $x, )*);
    };

    ($e:expr, $ctx:expr, $msg:expr $(,)?) => {
        option_unwrap!($e, $ctx, (), $msg);
    };
}

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
    PeerStateError(PeerStateError),
    #[fail(display = "Generic room error: {}", _0)]
    BadRoomSpec(String),
    #[fail(display = "Turn service error: {}", _0)]
    TurnServiceError(String),
}

impl From<PeerStateError> for RoomError {
    fn from(err: PeerStateError) -> Self {
        RoomError::PeerStateError(err)
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

// impl From<MembersLoadError> for RoomError {
//    fn from(err: MembersLoadError) -> Self {
//        RoomError::BadRoomSpec(format!(
//            "Error while loading room spec. {}",
//            err
//        ))
//    }
//}

/// Media server room with its [`Member`]s.
#[derive(Debug)]
pub struct Room {
    id: RoomId,

    /// [`Peer`]s of [`Member`]s in this [`Room`].
    peers: PeerRepository,

    pub pipeline: Pipeline,
}

impl Room {
    /// Create new instance of [`Room`].
    ///
    /// Returns [`RoomError::BadRoomSpec`] when error while [`Element`]
    /// transformation happens.
    pub fn new(
        room_spec: &RoomSpec,
        reconnect_timeout: Duration,
        turn: Box<dyn TurnAuthService>,
    ) -> Result<Self, RoomError> {
        Ok(Self {
            id: room_spec.id().clone(),
            peers: PeerRepository::from(HashMap::new()),
            pipeline: Pipeline::new(turn, reconnect_timeout, room_spec),
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
        self.pipeline.get_ice_servers(&member_id);
        let ice_servers = self
            .pipeline
            .get_ice_servers(&member_id)
            .ok_or_else(|| RoomError::NoTurnCredentials(member_id.clone()))?;

        let peer_created = Event::PeerCreated {
            peer_id: sender.id(),
            sdp_offer: None,
            tracks: sender.tracks(),
            ice_servers,
        };
        self.peers.add_peer(sender);
        Ok(Box::new(wrap_future(
            self.pipeline
                .send_event_to_participant(member_id, peer_created),
        )))
    }

    /// Sends [`Event::PeersRemoved`] to [`Member`].
    fn send_peers_removed(
        &mut self,
        member_id: MemberId,
        peers: Vec<PeerId>,
    ) -> ActFuture<(), RoomError> {
        Box::new(wrap_future(self.pipeline.send_event_to_participant(
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

        // TODO: better error
        let ice_servers = self
            .pipeline
            .get_ice_servers(&to_member_id)
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
            self.pipeline.send_event_to_participant(to_member_id, event),
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
            self.pipeline.send_event_to_participant(to_member_id, event),
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
            self.pipeline.send_event_to_participant(to_member_id, event),
        )))
    }

    /// Create [`Peer`]s between [`Member`]s and interconnect it by control
    /// API spec.
    fn connect_participants(
        &mut self,
        first_member: &MemberId,
        second_member: &MemberId,
        ctx: &mut <Self as Actor>::Context,
    ) {
        debug!(
            "Created peer member {} with member {}",
            first_member, second_member
        );

        let (first_peer_id, second_peer_id) = self.peers.create_peers(
            first_member,
            second_member,
            self.pipeline.endpoints_manager(),
        );

        self.connect_peers(ctx, first_peer_id, second_peer_id);
    }

    /// Create and interconnect all [`Peer`]s between connected [`Member`]
    /// and all available at this moment [`Member`].
    ///
    /// Availability is determines by checking [`RpcConnection`] of all
    /// [`Member`]s from [`WebRtcPlayEndpoint`]s and from receivers of
    /// connected [`Member`].
    fn init_participant_connections(
        &mut self,
        member_id: &MemberId,
        ctx: &mut <Self as Actor>::Context,
    ) {
        let participant_publishers =
            self.pipeline.get_publishers_by_member_id(member_id);
        // Create all connected publish endpoints.
        for (_, publish) in participant_publishers {
            for receiver in publish.borrow().sinks() {
                let q = self.pipeline.get_receiver_by_id(&receiver);
                let receiver = unit_option_unwrap!(
                    q,
                    ctx,
                    "Empty weak pointer for publisher receiver. {:?}.",
                    publish,
                );

                if self
                    .pipeline
                    .is_member_has_connection(&receiver.borrow().owner())
                    && !receiver.borrow().is_connected()
                {
                    self.connect_participants(
                        member_id,
                        &receiver.borrow().owner(),
                        ctx,
                    );
                }
            }
        }

        let member_receivers =
            self.pipeline.get_receivers_by_member_id(member_id);
        // Create all connected play's receivers peers.
        for (_, play) in member_receivers {
            let plays_publisher_id = {
                let q = self.pipeline.get_publisher_by_id(play.borrow().src());
                let play_publisher = unit_option_unwrap!(
                    q,
                    ctx,
                    "Empty weak pointer for play's publisher. {:?}.",
                    play,
                );
                let q = play_publisher.borrow().owner();
                q
            };

            if self.pipeline.is_member_has_connection(&plays_publisher_id)
                && !play.borrow().is_connected()
            {
                self.connect_participants(member_id, &plays_publisher_id, ctx);
            }
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

impl Handler<Authorize> for Room {
    type Result = Result<(), AuthorizationError>;

    /// Responses with `Ok` if `RpcConnection` is authorized, otherwise `Err`s.
    fn handle(
        &mut self,
        msg: Authorize,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        self.pipeline
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
        self.pipeline.peers_removed(&msg.peers_id);

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

    /// Saves new [`RpcConnection`] in [`MemberService`], initiates media
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
            .pipeline
            .connection_established(ctx, member_id.clone(), msg.connection)
            .map_err(|err, _, _| {
                error!("RpcConnectionEstablished error {:?}", err)
            })
            .map(move |participant, room, ctx| {
                room.init_participant_connections(
                    &participant.borrow().id(),
                    ctx,
                );
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
        let drop_fut = self.pipeline.drop_connections(ctx);
        ctx.wait(wrap_future(drop_fut));
    }
}

impl Handler<RpcConnectionClosed> for Room {
    type Result = ();

    /// Passes message to [`MemberService`] to cleanup stored connections.
    /// Remove all related for disconnected [`Member`] [`Peer`]s.
    fn handle(&mut self, msg: RpcConnectionClosed, ctx: &mut Self::Context) {
        info!(
            "RpcConnectionClosed for member {}, reason {:?}",
            msg.member_id, msg.reason
        );

        if let ClosedReason::Closed = msg.reason {
            self.peers.connection_closed(&msg.member_id, ctx);
        }

        self.pipeline
            .connection_closed(ctx, msg.member_id, msg.reason);
    }
}

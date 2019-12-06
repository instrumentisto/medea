//! Participant is [`Member`] with [`RpcConnection`]. [`ParticipantService`]
//! stores [`Member`]s and associated [`RpcConnection`]s, handles
//! [`RpcConnection`] authorization, establishment, message sending, Turn
//! credentials management.
//!
//! [`Member`]: crate::signalling::elements::member::Member
//! [`RpcConnection`]: crate::api::client::rpc_connection::RpcConnection
//! [`ParticipantService`]: crate::signalling::participants::ParticipantService

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use actix::{
    fut::wrap_future, ActorFuture, AsyncContext, Context, SpawnHandle,
};
use derive_more::Display;
use failure::Fail;
use futures::{
    future::{self, join_all, Either},
    Future,
};
use medea_client_api_proto::{CloseDescription, CloseReason, Event};

use crate::{
    api::{
        client::rpc_connection::{
            AuthorizationError, ClosedReason, EventMessage, RpcConnection,
            RpcConnectionClosed,
        },
        control::{
            refs::{Fid, ToEndpoint, ToMember},
            MemberId, RoomId, RoomSpec,
        },
    },
    log::prelude::*,
    media::IceUser,
    signalling::{
        elements::{
            member::MemberError, parse_members, Member, MembersLoadError,
        },
        room::{ActFuture, RoomError},
        Room,
    },
    turn::{TurnAuthService, TurnServiceErr, UnreachablePolicy},
    AppContext,
};

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Display, Fail)]
pub enum ParticipantServiceErr {
    /// Some error happened in [`TurnAuthService`].
    ///
    /// [`TurnAuthService`]: crate::turn::service::TurnAuthService
    #[display(fmt = "TurnService Error in ParticipantService: {}", _0)]
    TurnServiceErr(TurnServiceErr),

    /// [`Member`] with provided [`Fid`] not found.
    #[display(fmt = "Participant [id = {}] not found", _0)]
    ParticipantNotFound(Fid<ToMember>),

    /// [`Endpoint`] with provided URI not found.
    ///
    /// [`Endpoint`]: crate::signalling::elements::endpoints::Endpoint
    #[display(fmt = "Endpoint [id = {}] not found.", _0)]
    EndpointNotFound(Fid<ToEndpoint>),

    /// Some error happened in [`Member`].
    MemberError(MemberError),
}

impl From<TurnServiceErr> for ParticipantServiceErr {
    fn from(err: TurnServiceErr) -> Self {
        Self::TurnServiceErr(err)
    }
}

impl From<MemberError> for ParticipantServiceErr {
    fn from(err: MemberError) -> Self {
        Self::MemberError(err)
    }
}

/// Participant is [`Member`] with [`RpcConnection`]. [`ParticipantService`]
/// stores [`Member`]s and associated [`RpcConnection`]s, handles
/// [`RpcConnection`] authorization, establishment, message sending.
#[derive(Debug)]
pub struct ParticipantService {
    /// [`Room`]s id from which this [`ParticipantService`] was created.
    room_id: RoomId,

    /// [`Member`]s which currently are present in this [`Room`].
    members: HashMap<MemberId, Member>,

    /// Established [`RpcConnection`]s of [`Member`]s in this [`Room`].
    ///
    /// [`Member`]: crate::signalling::elements::member::Member
    // TODO: Replace Box<dyn RpcConnection>> with enum,
    //       as the set of all possible RpcConnection types is not closed.
    connections: HashMap<MemberId, Box<dyn RpcConnection>>,

    /// Stores [`RpcConnection`] drop tasks.
    /// If [`RpcConnection`] is lost, [`Room`] waits for connection_timeout
    /// before dropping it irrevocably in case it gets reestablished.
    drop_connection_tasks: HashMap<MemberId, SpawnHandle>,

    /// Reference to [`TurnAuthService`].
    turn_service: Arc<dyn TurnAuthService>,

    /// Duration, after which the server deletes the client session if
    /// the remote RPC client does not reconnect after it is idle.
    rpc_reconnect_timeout: Duration,
}

impl ParticipantService {
    /// Creates new [`ParticipantService`] from [`RoomSpec`].
    pub fn new(
        room_spec: &RoomSpec,
        context: &AppContext,
    ) -> Result<Self, MembersLoadError> {
        Ok(Self {
            room_id: room_spec.id().clone(),
            members: parse_members(room_spec)?,
            connections: HashMap::new(),
            drop_connection_tasks: HashMap::new(),
            turn_service: context.turn_service.clone(),
            rpc_reconnect_timeout: context.config.rpc.reconnect_timeout,
        })
    }

    /// Lookups [`Member`] by provided [`MemberId`].
    pub fn get_member_by_id(&self, id: &MemberId) -> Option<Member> {
        self.members.get(id).cloned()
    }

    /// Generates [`Fid`] which point to some [`Member`] in this
    /// [`ParticipantService`]'s [`Room`].
    ///
    /// __Note__ this function don't check presence of [`Member`] in
    /// [`ParticipantService`].
    pub fn get_fid_to_member(&self, member_id: MemberId) -> Fid<ToMember> {
        Fid::<ToMember>::new(self.room_id.clone(), member_id)
    }

    /// Lookups [`Member`] by [`MemberId`].
    ///
    /// Returns [`ParticipantServiceErr::ParticipantNotFound`] if [`Member`] not
    /// found.
    pub fn get_member(
        &self,
        id: &MemberId,
    ) -> Result<Member, ParticipantServiceErr> {
        self.members.get(id).cloned().map_or(
            Err(ParticipantServiceErr::ParticipantNotFound(
                self.get_fid_to_member(id.clone()),
            )),
            Ok,
        )
    }

    /// Returns all [`Member`] from this [`ParticipantService`].
    pub fn members(&self) -> HashMap<MemberId, Member> {
        self.members.clone()
    }

    /// Lookups [`Member`] by provided [`MemberId`] and credentials.
    ///
    /// Returns [`AuthorizationError::MemberNotExists`] if lookup by
    /// [`MemberId`] failed.
    ///
    /// Returns [`AuthorizationError::InvalidCredentials`] if [`Member`]
    /// was found, but incorrect credentials were provided.
    pub fn get_member_by_id_and_credentials(
        &self,
        member_id: &MemberId,
        credentials: &str,
    ) -> Result<Member, AuthorizationError> {
        match self.get_member_by_id(member_id) {
            Some(member) => {
                if member.credentials().eq(credentials) {
                    Ok(member)
                } else {
                    Err(AuthorizationError::InvalidCredentials)
                }
            }
            None => Err(AuthorizationError::MemberNotExists),
        }
    }

    /// Checks if [`Member`] has __active__ [`RpcConnection`].
    pub fn member_has_connection(&self, member_id: &MemberId) -> bool {
        self.connections.contains_key(member_id)
            && !self.drop_connection_tasks.contains_key(member_id)
    }

    /// Sends [`Event`] to specified remote [`Member`].
    pub fn send_event_to_member(
        &mut self,
        member_id: MemberId,
        event: Event,
    ) -> impl Future<Item = (), Error = RoomError> {
        match self.connections.get(&member_id) {
            Some(conn) => Either::A(
                conn.send_event(EventMessage::from(event))
                    .map_err(move |_| RoomError::UnableToSendEvent(member_id)),
            ),
            None => Either::B(future::err(RoomError::ConnectionNotExists(
                member_id,
            ))),
        }
    }

    /// Saves provided [`RpcConnection`], registers [`IceUser`].
    /// If [`Member`] already has any other [`RpcConnection`],
    /// then it will be closed.
    pub fn connection_established(
        &mut self,
        ctx: &mut Context<Room>,
        member_id: MemberId,
        conn: Box<dyn RpcConnection>,
    ) -> ActFuture<Member, ParticipantServiceErr> {
        let member = match self.get_member_by_id(&member_id) {
            None => {
                return Box::new(wrap_future(future::err(
                    ParticipantServiceErr::ParticipantNotFound(
                        self.get_fid_to_member(member_id),
                    ),
                )));
            }
            Some(member) => member,
        };

        // lookup previous member connection
        if let Some(mut connection) = self.connections.remove(&member_id) {
            debug!("Closing old RpcConnection for member [id = {}]", member_id);

            // cancel RpcConnection close task, since connection is
            // reestablished
            if let Some(handler) = self.drop_connection_tasks.remove(&member_id)
            {
                ctx.cancel_future(handler);
            }
            Box::new(wrap_future(
                connection
                    .close(CloseDescription::new(CloseReason::Reconnected))
                    .then(move |_| Ok(member)),
            ))
        } else {
            Box::new(
                wrap_future(self.turn_service.create(
                    member_id.clone(),
                    self.room_id.clone(),
                    UnreachablePolicy::ReturnErr,
                ))
                .map_err(|err, _: &mut Room, _| {
                    ParticipantServiceErr::from(err)
                })
                .and_then(
                    move |ice: IceUser, room: &mut Room, _| {
                        room.members.insert_connection(member_id, conn);
                        member.replace_ice_user(ice);

                        wrap_future(future::ok(member))
                    },
                ),
            )
        }
    }

    /// Inserts new [`RpcConnection`] into this [`ParticipantService`].
    fn insert_connection(
        &mut self,
        member_id: MemberId,
        conn: Box<dyn RpcConnection>,
    ) {
        self.connections.insert(member_id, conn);
    }

    /// If [`ClosedReason::Closed`], then removes [`RpcConnection`] associated
    /// with specified user [`Member`] from the storage and closes the room.
    /// If [`ClosedReason::Lost`], then creates delayed task that emits
    /// [`ClosedReason::Closed`].
    pub fn connection_closed(
        &mut self,
        member_id: MemberId,
        reason: &ClosedReason,
        ctx: &mut Context<Room>,
    ) {
        let closed_at = Instant::now();
        match reason {
            ClosedReason::Closed { .. } => {
                debug!("Connection for member [id = {}] removed.", member_id);
                self.connections.remove(&member_id);
                ctx.spawn(wrap_future(
                    self.delete_ice_user(&member_id).map_err(move |err| {
                        error!(
                            "Error deleting IceUser of Member [id = {}]. {:?}",
                            member_id, err
                        )
                    }),
                ));
                // TODO: we have no way to handle absence of RpcConnection right
                //       now.
            }
            ClosedReason::Lost => {
                self.drop_connection_tasks.insert(
                    member_id.clone(),
                    ctx.run_later(
                        self.rpc_reconnect_timeout,
                        move |room, ctx| {
                            info!(
                                "Member [id = {}] connection lost at {:?}. \
                                 Room [id = {}] will be be stopped.",
                                member_id,
                                closed_at,
                                room.id()
                            );
                            ctx.notify(RpcConnectionClosed {
                                member_id,
                                reason: ClosedReason::Closed { normal: false },
                            })
                        },
                    ),
                );
            }
        }
    }

    /// Deletes [`IceUser`] associated with provided [`Member`].
    fn delete_ice_user(
        &mut self,
        member_id: &MemberId,
    ) -> Box<dyn Future<Item = (), Error = TurnServiceErr>> {
        // TODO: rewrite using `Option::flatten` when it will be in stable rust.
        match self.get_member_by_id(&member_id) {
            Some(member) => match member.take_ice_user() {
                Some(ice_user) => self.turn_service.delete(vec![ice_user]),
                None => Box::new(future::ok(())),
            },
            None => Box::new(future::ok(())),
        }
    }

    /// Cancels all connection close tasks, closes all [`RpcConnection`]s and
    /// deletes all [`IceUser`]s.
    pub fn drop_connections(
        &mut self,
        ctx: &mut Context<Room>,
    ) -> impl Future<Item = (), Error = ()> {
        // canceling all drop_connection_tasks
        self.drop_connection_tasks.drain().for_each(|(_, handle)| {
            ctx.cancel_future(handle);
        });

        // closing all RpcConnection's
        let mut close_fut = self.connections.drain().fold(
            vec![],
            |mut futures, (_, mut connection)| {
                futures.push(
                    connection
                        .close(CloseDescription::new(CloseReason::Finished)),
                );
                futures
            },
        );

        // deleting all IceUsers
        let remove_ice_users = Box::new({
            let mut room_users = Vec::with_capacity(self.members.len());

            self.members.values().for_each(|data| {
                if let Some(ice_user) = data.take_ice_user() {
                    room_users.push(ice_user);
                }
            });
            self.turn_service
                .delete(room_users)
                .map_err(|err| error!("Error removing IceUsers {:?}", err))
        });
        close_fut.push(remove_ice_users);

        join_all(close_fut).map(|_| ())
    }

    /// Deletes [`Member`] from [`ParticipantService`], removes this user from
    /// [`TurnAuthService`], closes RPC connection with him and removes drop
    /// connection task.
    ///
    /// [`TurnAuthService`]: crate::turn::service::TurnAuthService
    pub fn delete_member(
        &mut self,
        member_id: &MemberId,
        ctx: &mut Context<Room>,
    ) {
        if let Some(drop) = self.drop_connection_tasks.remove(member_id) {
            ctx.cancel_future(drop);
        }

        if let Some(mut conn) = self.connections.remove(member_id) {
            ctx.spawn(wrap_future(
                conn.close(CloseDescription::new(CloseReason::Evicted)),
            ));
        }

        if let Some(member) = self.members.remove(member_id) {
            if let Some(ice_user) = member.take_ice_user() {
                let delete_ice_user_fut = self
                    .turn_service
                    .delete(vec![ice_user])
                    .map_err(|err| error!("Error removing IceUser {:?}", err));
                ctx.spawn(wrap_future(delete_ice_user_fut));
            }
        }
    }

    /// Inserts given [`Member`] into [`ParticipantService`].
    pub fn insert_member(&mut self, id: MemberId, member: Member) {
        self.members.insert(id, member);
    }

    /// Returns [`Iterator`] over [`MemberId`] and [`Member`] which this
    /// [`ParticipantRepository`] stores.
    pub fn iter_members(&self) -> impl Iterator<Item = (&MemberId, &Member)> {
        self.members.iter()
    }
}
